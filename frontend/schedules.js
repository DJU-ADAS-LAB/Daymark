(() => {
  'use strict';
  const $ = id => document.getElementById(id);
  const dialog = $('schedules-dialog');
  const periodFormat = new Intl.DateTimeFormat('ko-KR', { dateStyle: 'medium' });
  let editingId;
  let saving = false;
  let startDay;
  let endDay;

  function localInput(epoch) {
    const date = new Date(epoch);
    return new Date(epoch - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
  }

  function showStatus(message = '') {
    $('schedule-status').textContent = message;
    $('schedule-status').hidden = !message;
  }
  function showTimeFields(show) {
    const fields = $('schedule-time-fields');
    fields.hidden = !show;
    fields.disabled = !show;
  }

  function open(task, date = window.daymarkTasks.selectedDate(), lastDate = date) {
    if (dialog.open || saving) return;
    editingId = task?.id;
    const start = task?.start ?? +date;
    const deadline = new Date(lastDate);
    const now = new Date();
    deadline.setHours(Math.min(23, now.getHours() + 1), now.getHours() === 23 ? 59 : now.getMinutes(), 0, 0);
    const end = task?.end ?? +deadline;
    const [firstDate] = localInput(start).split('T');
    const [lastDay, lastTime] = localInput(end).split('T');
    startDay = firstDate;
    endDay = lastDay;
    $('schedule-period').textContent = periodFormat.formatRange(start, end);
    const plainTask = task && !Number.isFinite(task.start);
    $('schedule-add-time').hidden = !plainTask;
    $('schedule-use-time').checked = !plainTask;
    showTimeFields(!plainTask);
    $('schedules-heading').textContent = editingId ? (plainTask ? '할 일 수정' : '일정 수정') : '일정 추가';
    $('schedule-title').value = task?.text ?? '';
    $('schedule-end').value = lastTime;
    $('schedule-lead').value = String(task?.lead ?? 60);
    $('schedule-submit').textContent = editingId ? '변경 저장' : '일정 추가';
    showStatus();
    dialog.showModal();
    $('schedule-title').focus();
  }

  window.daymarkScheduleEditor = { open };
  $('add-task-schedule').addEventListener('click', () => open());
  $('schedule-use-time').addEventListener('change', event => showTimeFields(event.target.checked));

  $('schedule-form').addEventListener('submit', async event => {
    event.preventDefault();
    if (saving) return;
    const text = $('schedule-title').value.trim();
    if (!text) {
      showStatus('이름을 입력해 주세요.');
      return;
    }
    const record = { id: editingId, text, date: startDay };
    if (!$('schedule-time-fields').disabled) {
      const start = new Date(`${startDay}T00:00`).getTime();
      const end = new Date(`${endDay}T${$('schedule-end').value}`).getTime();
      if (!Number.isFinite(start) || !Number.isFinite(end) || start >= end) {
        showStatus('종료 시각을 확인해 주세요. 하루 일정은 자정 이후 시각을 선택해 주세요.');
        return;
      }
      Object.assign(record, { start, end, lead: Number($('schedule-lead').value) });
    }
    saving = true;
    $('schedule-submit').disabled = true;
    try {
      if (await window.daymarkTasks.saveItem(record)) {
        if (dialog.open) dialog.close();
      } else {
        showStatus('저장하지 못했습니다.');
      }
    } finally {
      saving = false;
      $('schedule-submit').disabled = false;
    }
  });
})();
