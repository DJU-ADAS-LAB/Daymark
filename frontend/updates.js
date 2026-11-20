(() => {
  'use strict';
  const api = window.__TAURI__;
  if (!api || api.window.getCurrentWindow().label !== 'main' || !api.updater) return;
  const button = document.getElementById('check-updates');
  let update;
  let busy = false;
  button.hidden = false;

  async function check(manual = false) {
    if (busy) return;
    busy = true;
    button.disabled = true;
    try {
      update = await api.updater.check({ timeout: 15000 });
      button.textContent = update ? `v${update.version} 설치 및 재시작` : (manual ? '최신 버전입니다' : '업데이트 확인');
      if (update) {
        const menu = document.getElementById('menu');
        menu.style.left = '';
        menu.style.top = '';
        menu.style.right = '8px';
        menu.style.top = '40px';
        menu.hidden = false;
      }
    } catch (error) {
      console.error('업데이트 확인 실패', error);
      button.textContent = manual ? '업데이트 확인 실패 · 다시 시도' : '업데이트 확인';
    } finally {
      busy = false;
      button.disabled = false;
    }
  }

  button.addEventListener('click', async event => {
    event.stopPropagation();
    if (busy) return;
    if (!update) return check(true);
    busy = true;
    button.disabled = true;
    button.textContent = '업데이트 다운로드 중…';
    try {
      await update.download();
      await api.core.invoke('prepare_update');
      button.textContent = '설치 중…';
      await update.install();
    } catch (error) {
      console.error('업데이트 설치 실패', error);
      button.textContent = '설치 실패 · 다시 시도';
      busy = false;
      button.disabled = false;
    }
  });
  if (!api.core.isTauri || api.core.isTauri()) check();
})();
