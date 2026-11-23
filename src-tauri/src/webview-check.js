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
    // Saved presets: the controls exist, and the backend really remembers and forgets a name
    // (read-only on the hardware — saving captures what the monitors report).
    assert(document.querySelector('#preset-list') && document.querySelector('#preset-name')
      && document.querySelector('#preset-save'), 'The preset controls are missing');
    // Under the update-hold test the app is paused, and a hardware action must be refused in plain
    // words rather than hang or half-run — so a refusal naming the pause is the correct answer there.
    try {
      const saved = await invoke('preset_save', { name: 'Hidden check' });
      assert(saved.includes('Hidden check'), 'Saving a preset did not list it');
      assert((await invoke('presets_list')).includes('Hidden check'), 'The saved preset is not listed');
      const left = await invoke('preset_delete', { name: 'Hidden check' });
      assert(!left.includes('Hidden check'), 'Deleting a preset did not remove it');
      result.presets = saved;
    } catch (error) {
      const message = String(error?.message ?? error);
      assert(/update|hold|paused/i.test(message), `Saving a preset failed for an unexpected reason: ${message}`);
      result.presets = `refused during the update hold: ${message}`;
    }
    assert(document.querySelector('#shortcuts').checked, 'Global shortcuts did not register');
    // The hidden instance never asks macOS, so the grant is absent: the panel must say so, by name,
    // with the button that opens the pane — a missing grant is a state, never a silent control.
    const note = document.querySelector('#accessibility-note');
    assert(note && !note.hidden, 'The Accessibility note is hidden while the grant is missing');
    assert(/Accessibility/.test(note.textContent) && note.querySelector('#open-accessibility'), 'The Accessibility note names no permission or has no button that opens the setting');
    assert(document.documentElement.scrollWidth <= innerWidth, 'Panel overflows horizontally');
    // body scrolls inside a capped height, so documentElement always reads the viewport — the body's
    // own scrollHeight is the one that says whether a person would have to scroll.
    assert(document.documentElement.scrollHeight <= innerHeight, `Panel overflows vertically: content ${document.documentElement.scrollHeight}px in a ${innerHeight}px dropdown`);
    assert(document.body.scrollHeight <= innerHeight + 1, `Panel content needs scrolling: ${document.body.scrollHeight}px of content in a ${innerHeight}px dropdown`);
    result.bounds = [innerWidth, innerHeight, document.documentElement.scrollHeight];
    result.ok = true;
  } catch (error) {
    result.ok = false;
    result.error = `${error?.message ?? error} / ${document.querySelector('#status').textContent}`;
  }
  await invoke('check_result', { result: JSON.stringify(result) });
})();
