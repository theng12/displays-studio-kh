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
    const label = version.textContent;
    assert(/^Displays Studio KH · v\d+\.\d+\.\d+$/.test(label), 'Bundle version missing');
    const shown = label.split('v')[1];
    assert(document.querySelector('#news-version').textContent === `v${shown}`, 'What\'s New names another version');
    version.click();
    assert(news.open && news.querySelectorAll('li').length >= 1, 'Release notes did not open');
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
    assert(document.documentElement.scrollHeight <= innerHeight, `Panel overflows vertically: content ${document.documentElement.scrollHeight}px in a ${innerHeight}px dropdown`);
    result.bounds = [innerWidth, innerHeight, document.documentElement.scrollHeight];
    result.ok = true;
  } catch (error) {
    result.ok = false;
    result.error = `${error.message} / ${document.querySelector('#status').textContent}`;
  }
  await invoke('check_result', { result: JSON.stringify(result) });
})();
