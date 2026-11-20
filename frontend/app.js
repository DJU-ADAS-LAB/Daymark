(() => {
  'use strict';
  const KEY = 'daymark.tasks.v1';
  const CHECKLIST_DATE = 'daymark.checklist.date';
  const DRAG_DEMO_SEEN = 'daymark.calendar.drag-demo.v1';
  const $ = id => document.getElementById(id);
  const pad = n => String(n).padStart(2, '0');
  const key = d => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const today = new Date();
  let selected = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  let viewing = new Date(selected.getFullYear(), selected.getMonth(), 1);
  let tasks = [];
  let tasksLoaded = false;
  let saveQueue = Promise.resolve();
  let openingChecklist = false;
  const bridge = window.daymarkBridge;
  const nativeWindow = window.__TAURI__?.window.getCurrentWindow();
  const isChecklistWindow = nativeWindow?.label === 'checklist';
  if (nativeWindow) document.body.classList.add('native-window');
  const id = () => globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
  const validDate = value => /^\d{4}-\d{2}-\d{2}$/.test(value) && !Number.isNaN(new Date(`${value}T12:00:00`).getTime());
  const isScheduled = row => Number.isSafeInteger(row.start) && Number.isSafeInteger(row.end)
    && row.start < row.end && [0, 60, 120].includes(row.lead);
  const normalize = row => ({
    id: String(row.id || id()), text: row.text.trim().slice(0, 200),
    date: isScheduled(row) ? key(new Date(row.start)) : validDate(row.date) ? row.date : key(today),
    done: !!row.done,
    ...(isScheduled(row) ? { start: row.start, end: row.end, lead: row.lead } : {})
  });
  function fallsOn(task, date) {
    if (!isScheduled(task)) return task.date === key(date);
    const day = new Date(date.getFullYear(), date.getMonth(), date.getDate());
    const nextDay = new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1);
    return task.start < +nextDay && task.end > +day;
  }
  const readRows = value => {
    const rows = Array.isArray(value) ? value : value?.tasks;
    if (!Array.isArray(rows)) throw new Error('할 일 데이터가 아닙니다.');
    if (rows.some(row => !row || typeof row.text !== 'string' || !row.text.trim() || (row.date && !validDate(row.date)))) throw new Error('파일에 잘못된 항목이 있습니다.');
    return rows.map(normalize);
  };
  function showStatus(message = '') {
    const status = $(isChecklistWindow ? 'task-save-error' : 'save-state');
    status.textContent = message;
    status.hidden = !message;
  }
  async function load() {
    const queued = saveQueue;
    try {
      await queued;
      let value;
      if (nativeWindow) {
        let legacy = null;
        try {
          if (!tasksLoaded) {
            const stored = localStorage.getItem(KEY);
            if (stored !== null) legacy = JSON.stringify(readRows(JSON.parse(stored)));
          }
        } catch (error) {
          console.error('기존 할 일 데이터를 읽지 못했습니다.', error);
        }
        const data = await window.__TAURI__.core.invoke('load_document', { document: 'tasks', legacy });
        value = JSON.parse(data ?? '[]');
      } else {
        value = bridge?.loadTasks ? await bridge.loadTasks() : JSON.parse(localStorage.getItem(KEY) || '[]');
      }
      if (queued !== saveQueue) return;
      tasks = readRows(value);
      if (!nativeWindow && !bridge && localStorage.getItem('daymark.schedules.v1') !== null) {
        const old = JSON.parse(localStorage.getItem('daymark.schedules.v1'));
        const ids = new Set(tasks.map(task => task.id));
        for (const item of old) {
          if (!ids.has(item.id) && isScheduled(item) && item.title?.trim()) {
            tasks.push(normalize({ ...item, text: item.title }));
            ids.add(item.id);
          }
        }
        localStorage.setItem(KEY, JSON.stringify(tasks));
        localStorage.removeItem('daymark.schedules.v1');
      }
      tasksLoaded = true;
      $('task-input').disabled = false;
      $('import').disabled = false;
      $('add-task-schedule').disabled = false;
    } catch (error) { showStatus('불러오기 실패'); console.error(error); }
    render();
  }
  function persist() {
    const snapshot = JSON.stringify(tasks);
    saveQueue = saveQueue.catch(() => { }).then(async () => {
      if (nativeWindow) {
        await window.__TAURI__.core.invoke('save_document', { document: 'tasks', data: snapshot });
      } else if (bridge?.saveTasks) await bridge.saveTasks(JSON.parse(snapshot));
      else localStorage.setItem(KEY, snapshot);
      showStatus();
      return true;
    }).catch(error => { showStatus('저장 실패'); console.error(error); return false; });
    return saveQueue;
  }
  let weekCount = 0;
  let visibleRibbonLanes = 1;
  function ribbonCapacity(grid, weeks) {
    return grid.clientHeight / weeks >= 54 ? 2 : 1;
  }

  function renderCalendar() {
    $('year-label').textContent = `${viewing.getFullYear()}년`;
    $('month-label').textContent = `${viewing.getMonth() + 1}월`;
    const grid = $('grid'); grid.replaceChildren();
    const first = new Date(viewing.getFullYear(), viewing.getMonth(), 1).getDay();
    const last = new Date(viewing.getFullYear(), viewing.getMonth() + 1, 0).getDate();
    const trailing = 7 - (first + last) % 7;
    const firstVisible = 1 - first;
    const lastVisible = last + trailing;
    const dateAt = day => new Date(viewing.getFullYear(), viewing.getMonth(), day);
    const scheduled = tasks.filter(isScheduled).sort((a, b) => a.start - b.start || b.end - a.end);
    const ribbonTasks = [...scheduled, ...tasks.filter(task => !isScheduled(task))];
    const cells = [];

    for (let n = firstVisible; n <= lastVisible; n++) {
      const date = dateAt(n), dateId = key(date);
      const daily = tasks.filter(task => fallsOn(task, date));
      const total = daily.length;
      const remaining = daily.filter(task => !task.done).length;
      const scheduleCount = daily.filter(isScheduled).length;
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'day';
      if (n < 1 || n > last) b.classList.add('outside-day');
      b.dataset.day = n;
      const number = document.createElement('span');
      number.className = 'day-number';
      number.textContent = date.getDate();
      b.append(number);
      if (dateId === key(selected)) b.classList.add('selected');
      if (dateId === key(today)) {
        b.classList.add('today');
        b.setAttribute('aria-current', 'date');
      }
      b.setAttribute('aria-label', `${dateId}, 할 일 ${total}개, 미완료 ${remaining}개, 일정 ${scheduleCount}개`);
      b.setAttribute('aria-pressed', String(dateId === key(selected)));
      if (!nativeWindow) b.setAttribute('aria-controls', 'task-panel');
      b.addEventListener('click', () => openChecklist(date));
      grid.append(b);
      cells.push(b);
    }
    const weeks = (first + last + trailing) / 7;
    weekCount = weeks;
    visibleRibbonLanes = ribbonCapacity(grid, weeks);
    grid.style.setProperty('--day-content-height', `${22 + visibleRibbonLanes * 16}px`);
    const overlay = document.createElement('div');
    overlay.className = 'schedule-ribbons';
    overlay.setAttribute('aria-hidden', 'true');
    overlay.style.gridTemplateRows = `repeat(${weeks}, minmax(0, 1fr))`;
    for (let week = 0; week < weeks; week++) {
      const firstInWeek = firstVisible + week * 7;
      const endDay = firstInWeek + 6;
      const lanes = Array(visibleRibbonLanes).fill(firstInWeek - 1);
      const overflow = new Map();
      for (const task of ribbonTasks) {
        let from = firstInWeek;
        while (from <= endDay && !fallsOn(task, dateAt(from))) from++;
        if (from > endDay) continue;
        let to = from;
        while (to < endDay && fallsOn(task, dateAt(to + 1))) to++;
        const lane = lanes.findIndex(previousEnd => previousEnd < from);
        if (lane < 0) {
          for (let day = from; day <= to; day++) overflow.set(day, (overflow.get(day) ?? 0) + 1);
          continue;
        }
        lanes[lane] = to;
        const ribbon = document.createElement('span');
        ribbon.className = 'schedule-ribbon';
        if (task.done) ribbon.classList.add('complete');
        if (fallsOn(task, dateAt(from - 1))) ribbon.classList.add('continues-from');
        if (fallsOn(task, dateAt(to + 1))) ribbon.classList.add('continues-to');
        ribbon.style.gridRow = week + 1;
        ribbon.style.gridColumn = `${from - firstInWeek + 1} / ${to - firstInWeek + 2}`;
        ribbon.style.setProperty('--lane-offset', `${22 + lane * 16}px`);
        ribbon.textContent = task.text;
        overlay.append(ribbon);
      }
      for (const [day, count] of overflow) cells[day - firstVisible].dataset.extraSchedules = `+${count}`;
    }
    grid.append(overlay);
  }

  const calendarGrid = $('grid');
  let rangeDrag;
  let suppressDateClick = false;
  let demoDismissed = false;
  let demoTimer;
  function dismissDragDemo() {
    demoDismissed = true;
    clearTimeout(demoTimer);
    calendarGrid.querySelector('.calendar-drag-demo')?.remove();
  }

  function rememberDragDemo() {
    try {
      localStorage.setItem(DRAG_DEMO_SEEN, '1');
    } catch (_) { /* The hint can still run when storage is unavailable. */ }
  }

  function showDragDemo() {
    if (isChecklistWindow || !tasksLoaded || demoDismissed) return;
    try {
      if (localStorage.getItem(DRAG_DEMO_SEEN)) return;
    } catch (_) { /* Rendering the hint does not require storage. */ }
    const days = [...calendarGrid.querySelectorAll('.day:not(.outside-day)')];
    const index = days.findIndex((day, i) => days[i + 2]?.offsetTop === day.offsetTop);
    if (index < 0) return;
    const first = days[index].getBoundingClientRect();
    const last = days[index + 2].getBoundingClientRect();
    const area = calendarGrid.getBoundingClientRect();
    if (!area.width || !area.height) return;
    const demo = document.createElement('div');
    demo.className = 'calendar-drag-demo';
    demo.style.left = `${first.left - area.left}px`;
    demo.style.top = `${first.top - area.top}px`;
    demo.style.width = `${last.right - first.left}px`;
    demo.style.height = `${first.height}px`;
    for (let step = 0; step < 3; step++) {
      const cell = document.createElement('span');
      cell.className = 'calendar-drag-demo-cell';
      cell.style.setProperty('--demo-delay', `${step * 230}ms`);
      demo.append(cell);
    }
    const hint = document.createElement('span');
    hint.className = 'calendar-drag-demo-text';
    hint.textContent = '날짜를 드래그해 일정 추가';
    demo.append(hint);
    calendarGrid.append(demo);
    rememberDragDemo();
    demoTimer = setTimeout(dismissDragDemo, 4200);
  }
  if (!isChecklistWindow) {
    new ResizeObserver(() => {
      if (weekCount && !rangeDrag && ribbonCapacity(calendarGrid, weekCount) !== visibleRibbonLanes) {
        renderCalendar();
      }
    }).observe(calendarGrid);
  }
  calendarGrid.addEventListener('pointerdown', event => {
    const cell = event.target.closest('.day');
    if (!tasksLoaded || !event.isPrimary || event.button !== 0 || !cell) return;
    dismissDragDemo();
    suppressDateClick = false;
    rangeDrag = { pointer: event.pointerId, first: Number(cell.dataset.day), last: Number(cell.dataset.day), moved: false };
  });
  window.addEventListener('pointermove', event => {
    if (!rangeDrag || rangeDrag.pointer !== event.pointerId) return;
    const cell = document.elementFromPoint(event.clientX, event.clientY)?.closest('.day');
    if (!cell || !calendarGrid.contains(cell)) return;
    const day = Number(cell.dataset.day);
    if (day === rangeDrag.last) return;
    rangeDrag.last = day;
    if (!rangeDrag.moved) {
      rangeDrag.moved = true;
      rememberDragDemo();
      calendarGrid.setPointerCapture(event.pointerId);
    }
    const from = Math.min(rangeDrag.first, day), to = Math.max(rangeDrag.first, day);
    for (const button of calendarGrid.querySelectorAll('.day')) {
      const number = Number(button.dataset.day);
      button.classList.toggle('range-preview', number >= from && number <= to);
    }
  });
  function endDrag(event) {
    if (!rangeDrag || rangeDrag.pointer !== event.pointerId) return;
    const { first, last, moved } = rangeDrag;
    rangeDrag = undefined;
    calendarGrid.querySelectorAll('.range-preview').forEach(cell => cell.classList.remove('range-preview'));
    if (!moved || event.type !== 'pointerup') return;
    suppressDateClick = true;
    const year = viewing.getFullYear(), month = viewing.getMonth();
    const start = new Date(year, month, Math.min(first, last));
    const end = new Date(year, month, Math.max(first, last));
    setTimeout(() => window.daymarkScheduleEditor.open(undefined, start, end), 0);
  }
  window.addEventListener('pointerup', endDrag);
  window.addEventListener('pointercancel', endDrag);
  calendarGrid.addEventListener('click', event => {
    if (!suppressDateClick || event.detail === 0) return;
    suppressDateClick = false;
    event.stopPropagation();
  }, true);
  const scheduleTime = new Intl.DateTimeFormat('ko-KR', { dateStyle: 'short', timeStyle: 'short' });
  function renderTasks() {
    const daily = tasks.filter(task => fallsOn(task, selected));
    const done = daily.filter(t => t.done).length;
    $('date-label').textContent = new Intl.DateTimeFormat('ko-KR', { month: 'long', day: 'numeric', weekday: 'long' }).format(selected);
    $('progress').setAttribute('aria-valuenow', daily.length ? Math.round(done / daily.length * 100) : 0);
    $('progress-fill').style.width = `${daily.length ? done / daily.length * 100 : 0}%`;
    $('task-count').textContent = `할 일 ${daily.length}개`;
    const list = $('tasks'); list.replaceChildren();
    if (!daily.length) { const empty = document.createElement('li'); empty.className = 'empty'; empty.innerHTML = '<strong>아직 할 일이 없어요</strong><span>위 입력칸에 첫 할 일을 적어보세요.</span>'; list.append(empty); return; }
    for (const task of daily) {
      const li = document.createElement('li');
      const check = document.createElement('input'); check.type = 'checkbox'; check.checked = task.done; check.setAttribute('aria-label', `${task.text} 완료`);
      check.addEventListener('change', () => { task.done = check.checked; persist(); render(); });
      const label = document.createElement('span'); label.className = `task-text${task.done ? ' done' : ''}`; label.textContent = task.text;
      if (isScheduled(task)) {
        const range = document.createElement('small');
        range.className = 'task-time-range';
        range.textContent = `종료 ${scheduleTime.format(task.end)}`;
        label.append(range);
      }
      const edit = document.createElement('button'); edit.type = 'button'; edit.className = 'task-action'; edit.textContent = '✎'; edit.setAttribute('aria-label', `${task.text} 수정`);
      edit.addEventListener('click', () => window.daymarkScheduleEditor.open(task, selected));
      const del = document.createElement('button'); del.type = 'button'; del.className = 'task-action'; del.textContent = '×'; del.setAttribute('aria-label', `${task.text} 삭제`);
      del.addEventListener('click', () => { tasks = tasks.filter(t => t.id !== task.id); persist(); render(); });
      li.append(check, label, edit, del); list.append(li);
    }
  }
  function render() { renderCalendar(); renderTasks(); }
  window.daymarkTasks = {
    selectedDate: () => selected,
    async saveItem(record) {
      if (!tasksLoaded) return false;
      const index = tasks.findIndex(task => task.id === record.id);
      if (record.id && index < 0) return false;
      const previous = tasks[index];
      const task = normalize({ ...record, done: previous?.done });
      if (index < 0) tasks.unshift(task);
      else tasks[index] = task;
      const saved = await persist();
      if (!saved && tasks.includes(task)) {
        if (previous) tasks[tasks.indexOf(task)] = previous;
        else tasks = tasks.filter(item => item !== task);
      }
      render();
      return saved;
    }
  };
  async function openChecklist(date) {
    const dateId = key(date);
    const sameDate = key(selected) === dateId;
    selected = date;
    render();
    try {
      if (nativeWindow) {
        const toggle = localStorage.getItem(CHECKLIST_DATE) === dateId;
        localStorage.setItem(CHECKLIST_DATE, dateId);
        if (!openingChecklist) {
          openingChecklist = true;
          try {
            await window.__TAURI__.core.invoke('open_checklist', { toggle });
          } finally {
            openingChecklist = false;
          }
        }
      } else {
        if (sameDate && !$('task-panel').hidden) {
          closePanel();
          return;
        }
        $('task-panel').hidden = false;
        $('task-input').focus();
      }
    } catch (error) {
      showStatus('체크리스트 열기 실패');
      console.error(error);
    }
  }
  async function closePanel() {
    if (isChecklistWindow) {
      await saveQueue;
      try {
        await nativeWindow.close();
      } catch (error) {
        console.error('체크리스트 창을 닫지 못했습니다.', error);
      }
      return;
    }
    $('task-panel').hidden = true;
    ($('grid').querySelector('.selected') ?? $('prev')).focus();
  }
  function closeMenu() { $('menu').hidden = true; }
  $('month').addEventListener('contextmenu', event => {
    event.preventDefault();
    const menu = $('menu');
    menu.hidden = false;
    menu.style.left = `${Math.max(0, Math.min(event.clientX, innerWidth - menu.offsetWidth - 4))}px`;
    menu.style.top = `${Math.max(0, Math.min(event.clientY, innerHeight - menu.offsetHeight - 4))}px`;
  });
  $('task-form').addEventListener('submit', e => { e.preventDefault(); const input = $('task-input'), text = input.value.trim(); if (!tasksLoaded || !text) return; tasks.unshift({ id: id(), text, date: key(selected), done: false }); input.value = ''; persist(); render(); input.focus(); });
  $('close-panel').addEventListener('click', closePanel);
  let browserMemo;
  $('open-memo').addEventListener('click', async event => {
    const button = event.currentTarget;
    try {
      if (!nativeWindow) {
        if (browserMemo && !browserMemo.closed) {
          browserMemo.close();
          browserMemo = null;
          return;
        }
        browserMemo = window.open('memo.html', 'daymark-memo', 'popup,width=340,height=510');
        if (!browserMemo) {
          throw new Error('팝업을 열 수 없습니다.');
        }
        return;
      }
      button.disabled = true;
      await window.__TAURI__.core.invoke('toggle_memo');
    } catch (error) {
      showStatus('메모 열기·닫기 실패');
      console.error(error);
    } finally {
      button.disabled = false;
    }
  });
  $('prev').addEventListener('click', () => { viewing = new Date(viewing.getFullYear(), viewing.getMonth() - 1, 1); renderCalendar(); });
  $('next').addEventListener('click', () => { viewing = new Date(viewing.getFullYear(), viewing.getMonth() + 1, 1); renderCalendar(); });
  $('today').addEventListener('click', () => { selected = new Date(today.getFullYear(), today.getMonth(), today.getDate()); viewing = new Date(selected.getFullYear(), selected.getMonth(), 1); render(); closeMenu(); });
  $('import').addEventListener('click', () => { $('import-file').click(); closeMenu(); });
  $('import-file').addEventListener('change', async e => { const file = e.target.files[0]; if (!tasksLoaded || !file) return; try { const rows = readRows(JSON.parse(await file.text())); const ids = new Set(tasks.map(t => t.id)); for (const row of rows) { if (ids.has(row.id)) continue; tasks.push(row); ids.add(row.id); } persist(); render(); } catch (error) { showStatus('가져오기 실패'); console.error(error); } e.target.value = ''; });
  $('export').addEventListener('click', () => { const blob = new Blob([JSON.stringify({ app: 'daymark', version: 1, tasks }, null, 2)], { type: 'application/json' }), url = URL.createObjectURL(blob), a = document.createElement('a'); a.href = url; a.download = 'daymark-tasks.json'; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); closeMenu(); });
  $('close').addEventListener('click', async () => {
    try {
      await saveQueue;
      if (bridge?.closeWindow) {
        await bridge.closeWindow();
      } else if (window.__TAURI__) {
        await window.__TAURI__.window.getCurrentWindow().close();
      } else {
        window.close()
      }
    }
    catch (error) {
      console.error('창을 닫지 못했습니다', error);
    }
  })


  document.addEventListener('click', e => { if (!e.target.closest('.menu')) closeMenu(); });
  document.addEventListener('keydown', e => {
    if (e.key !== 'Escape') return;
    if ($('schedules-dialog').open) return;
    if (!$('menu').hidden) closeMenu();
    else if (!$('task-panel').hidden) closePanel();
  });
  function selectChecklistDate(value) {
    if (validDate(value)) selected = new Date(`${value}T12:00:00`);
  }

  window.addEventListener('storage', event => {
    if (!nativeWindow && event.key === KEY) load();
    if (isChecklistWindow && event.key === CHECKLIST_DATE) {
      selectChecklistDate(event.newValue);
      renderTasks();
      $('task-input').focus();
    }
  });

  if (isChecklistWindow) {
    document.body.classList.add('checklist-window');
    selectChecklistDate(localStorage.getItem(CHECKLIST_DATE));
    $('task-panel').hidden = false;
  }
  $('task-input').disabled = true;
  $('import').disabled = true;
  async function start() {
    if (nativeWindow) {
      try {
        await nativeWindow.listen('daymark-tasks-changed', load);
      } catch (error) {
        console.error('할 일 변경 알림을 받지 못했습니다.', error);
      }
    }
    await load();
    if (isChecklistWindow) $('task-input').focus();
    else demoTimer = setTimeout(showDragDemo, 550);
  }
  start();
})();
