// Evaluated only in a feature-gated hidden WKWebView; no screen or keyboard input.
(async () => {
  if (document.body.dataset.ready !== 'true') {
    await new Promise((resolve) => window.addEventListener('displays-ready', resolve, { once: true }));
  }
  const { invoke } = window.__TAURI__.core;
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const result = {};
  try {
    const version = document.querySelector('#version');
    const news = document.querySelector('#news');
    assert(version.textContent.includes('v0.1.1'), 'Bundle version missing');
    version.click();
    assert(news.open && news.querySelectorAll('li').length === 5, 'Release notes did not open');
    news.close();
    const range = [...document.querySelectorAll('input[type=range]')];
    result.displays = [...document.querySelectorAll('main h2')].map((node) => node.textContent);
    result.settings = range.filter((node) => node.id !== 'all-displays').map((node) => [node.getAttribute('aria-label'), node.valueAsNumber]);
    assert(result.displays.includes('MSI MP273') && [16, 18, 98].every((code) => range.some((node) => Number(node.dataset.code) === code)), 'Expected MSI hardware controls are missing');
    assert(document.querySelector('#all-displays'), 'The all-displays slider is missing');
    assert(document.querySelector('#login') && document.querySelector('#brightness-keys'), 'The login and brightness-key switches are missing');
    assert(document.querySelector('#intro'), 'The first-run explanation is missing');
    assert(document.querySelector('#shortcuts').checked, 'Global shortcuts did not register');
    assert(document.documentElement.scrollWidth <= innerWidth, 'Panel overflows horizontally');
    result.bounds = [innerWidth, innerHeight, document.documentElement.scrollHeight];
    result.ok = true;
  } catch (error) {
    result.ok = false;
    result.error = `${error.message} / ${document.querySelector('#status').textContent}`;
  }
  await invoke('check_result', { result: JSON.stringify(result) });
})();
