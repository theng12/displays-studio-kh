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
    result.controls = result.settings.map(([name, value]) => [name, value]);
    // Whatever this Mac reports, every reported level must have its own slider and its own label —
    // naming one monitor here would make the check fail on a sleeping or swapped display instead of
    // on a fault (2026-11-24: a virtual display replaced the MSI for a few minutes).
    for (const node of result.controls) {
      assert(Number.isInteger(Number(node[1])) && /for /.test(node[0]), `A reported control has no label or code: ${node}`);
    }
    assert(!range.some((node) => node.id !== 'all-displays' && !node.dataset.code), 'A level slider carries no code');
    assert(document.querySelector('#all-displays'), 'The all-displays slider is missing');
    // The switches moved to the Settings window; the panel keeps one way in, and the settings commands
    // answer with the real state.
    assert(document.querySelector('#settings-open'), 'The Settings button is missing from the panel');
    const settingsInfo = await invoke('settings_info');
    assert(settingsInfo.follow === 'pointer' && Array.isArray(settingsInfo.hotkeys),
      'settings_info did not default to pointer follow and return the hotkey list');
    const followed = await invoke('set_keys_follow', { follow: 'selected', display: 'demo-display' });
    assert(followed.follow === 'selected' && followed.selected === 'demo-display', 'The keys-follow choice did not stick');
    const allMode = await invoke('set_keys_follow', { follow: 'all', display: '' });
    assert(allMode.follow === 'all', 'The keys-follow choice did not switch to all');
    const back = await invoke('set_keys_follow', { follow: 'pointer', display: '' });
    assert(back.follow === 'pointer', 'The keys-follow choice did not go back to pointer');
    result.settings_screen = { follow: settingsInfo.follow, hotkeys: settingsInfo.hotkeys.length };
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
      // Apply it while it is still saved: the values are the ones just read, so the write path runs
      // end to end against the real monitor and confirms without moving it.
      const applied = await invoke('preset_apply', { name: 'Hidden check' });
      assert(applied.changed.length >= 1, `Applying the preset reached no display: ${applied.message}`);
      result.preset_apply = applied.message;
      const left = await invoke('preset_delete', { name: 'Hidden check' });
      assert(!left.includes('Hidden check'), 'Deleting a preset did not remove it');
      result.presets = saved;
    } catch (error) {
      const message = String(error?.message ?? error);
      if (/Nothing to save/.test(message)) {
        // No monitor reported a setting when this ran: an environment fact the app names in words.
        result.presets = `no monitor reported a setting: ${message}`;
      } else {
      assert(/update|hold|paused/i.test(message), `Saving a preset failed for an unexpected reason: ${message}`);
      // The person is told why, in the panel's own status line — measured by pressing the button, not
      // by reading the handler.
      document.querySelector('#preset-name').value = 'Hidden check';
      document.querySelector('#preset-save').click();
      await new Promise((resolve) => setTimeout(resolve, 400));
      const shown = document.querySelector('#status').textContent;
      assert(/holding new work|paused|update/i.test(shown), `The panel did not explain the paused save: ${shown}`);
      result.presets = `refused during the update hold, and the panel said: ${shown}`;
      }
    }
    // The hidden instance never asks macOS, so the grant is absent: the panel must say so, by name,
    // with the button that opens the pane — a missing grant is a state, never a silent control.
    const note = document.querySelector('#accessibility-note');
    assert(note && !note.hidden, 'The Accessibility note is hidden while the grant is missing');
    assert(/Brightness keys need Accessibility/.test(note.textContent) && note.querySelector('#open-accessibility'), 'The Accessibility note does not show "Brightness keys need Accessibility" or its button');
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
