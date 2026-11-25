const { invoke } = window.__TAURI__.core;
const $ = (selector) => document.querySelector(selector);
const element = (tag, text) => {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
};

let info = null;
let displays = [];
let recording = null;      // { display, brighter, button } while waiting for keys
let busy = false;

function status(text, error = false) {
  $('#status').textContent = text;
  $('#status').toggleAttribute('data-error', error);
}

function label(chord) {
  // "Control+Shift+KeyB" reads as ⌃⇧B; the stored form never changes, only what is shown.
  return chord
    .replace('Control', '⌃').replace('Shift', '⇧').replace('Alt', '⌥').replace('Super', '⌘')
    .replace(/\+?Key([A-Z])/, '$1').replace(/\+?Digit([0-9])/, '$1')
    .replace(/\+/g, '');
}

function chordFor(display, brighter) {
  return info.hotkeys.find((hotkey) => hotkey.display === display && hotkey.brighter === brighter)?.chord;
}

function render() {
  $('#version').textContent = `Displays Studio KH ${info.version}`;
  $('#shortcuts').checked = info.shortcuts.enabled;
  $('#shortcuts').disabled = false;
  $('#brightness-keys').checked = info.keys.enabled;
  $('#brightness-keys').disabled = false;
  $('#login').checked = info.login.enabled;
  $('#login').disabled = false;
  $('#accessibility').hidden = info.keys.trusted;
  $('#follow-pointer').checked = info.follow !== 'all' && info.follow !== 'selected';
  $('#follow-all').checked = info.follow === 'all';
  $('#follow-selected').checked = info.follow === 'selected';

  const selected = $('#selected-display');
  selected.replaceChildren(...(displays.length
    ? displays.map((display) => { const option = element('option', display.name); option.value = display.id; return option; })
    : [Object.assign(element('option', 'No external display found'), { value: '' })]));
  if (displays.some((display) => display.id === info.selected)) selected.value = info.selected;
  selected.disabled = info.follow !== 'selected';

  const rows = displays.length ? displays.map((display) => {
    const row = element('div');
    row.className = 'hotkey-row';
    row.append(element('span', display.name));
    for (const brighter of [true, false]) {
      const button = element('button');
      button.type = 'button';
      const chord = chordFor(display.id, brighter);
      button.textContent = chord ? label(chord) : (brighter ? 'Set brighter…' : 'Set dimmer…');
      button.title = brighter ? 'Brighter shortcut' : 'Dimmer shortcut';
      button.addEventListener('click', () => startRecording(display.id, brighter, button));
      const clear = element('button', 'Clear');
      clear.type = 'button';
      clear.disabled = !chord;
      clear.addEventListener('click', () => saveChord(display.id, brighter, null));
      const cell = element('span');
      cell.className = 'chord';
      cell.append(button, ' ', clear);
      row.append(cell);
      if (brighter) row.dataset.mid = display.id;
    }
    return row;
  }) : [element('p', 'No external display is connected.')];
  $('#hotkeys').replaceChildren(...rows);
  const errors = [info.shortcuts.error, info.keys.error, info.login.error].filter(Boolean);
  if (errors.length) status(errors.join(' '), true);
}

function startRecording(display, brighter, button) {
  if (recording) recording.button.textContent = recording.previous;
  recording = { display, brighter, button, previous: button.textContent };
  button.textContent = 'Press the keys…';
  button.classList.add('recording');
  status('Press the shortcut you want, or Escape to cancel.');
}

function modifierNames(event) {
  const names = [];
  if (event.ctrlKey) names.push('Control');
  if (event.shiftKey) names.push('Shift');
  if (event.altKey) names.push('Alt');
  if (event.metaKey) names.push('Super');
  return names;
}

window.addEventListener('keydown', async (event) => {
  if (!recording) return;
  event.preventDefault();
  if (event.key === 'Escape') {
    const { button, previous } = recording;
    recording = null;
    button.textContent = previous;
    button.classList.remove('recording');
    status('Left as it was.');
    return;
  }
  const codes = modifierNames(event);
  if (!codes.length || /^(Control|Shift|Alt|Meta|Super)/.test(event.code)) return;   // a bare modifier is not a shortcut
  const chord = [...codes, event.code].join('+');
  const { display, brighter, button } = recording;
  recording = null;
  button.classList.remove('recording');
  await saveChord(display, brighter, chord);
});

async function saveChord(display, brighter, chord) {
  if (busy) return;
  busy = true;
  status(chord ? 'Registering the shortcut…' : 'Removing the shortcut…');
  try {
    info = chord
      ? await invoke('set_hotkey', { display, brighter, chord })
      : await invoke('clear_hotkey', { display, brighter });
    render();
    status(chord ? `Saved ${label(chord)}.` : 'Shortcut removed.');
  } catch (error) {
    status(String(error), true);
    try { info = await invoke('settings_info'); render(); } catch { /* keep the message */ }
  } finally {
    busy = false;
  }
}

async function saveFollow() {
  const follow = $('#follow-selected').checked ? 'selected' : ($('#follow-all').checked ? 'all' : 'pointer');
  try {
    info = await invoke('set_keys_follow', { follow, display: $('#selected-display').value });
    render();
    const msg = follow === 'selected'
      ? 'The keys move the selected display.'
      : (follow === 'all' ? 'The keys move every external display.' : 'The keys move the display under the pointer.');
    status(msg);
  } catch (error) {
    status(String(error), true);
  }
}

$('#follow-pointer').addEventListener('change', saveFollow);
$('#follow-all').addEventListener('change', saveFollow);
$('#follow-selected').addEventListener('change', saveFollow);
$('#selected-display').addEventListener('change', saveFollow);
$('#open-accessibility').addEventListener('click', () => {
  invoke('open_accessibility_settings').catch((error) => status(String(error), true));
});
$('#done').addEventListener('click', () => invoke('close_panel').catch(() => window.close()));
$('#shortcuts').addEventListener('change', async () => {
  const requested = $('#shortcuts').checked;
  $('#shortcuts').disabled = true;
  try {
    const setting = await invoke('set_shortcuts', { enabled: requested });
    $('#shortcuts').checked = setting.enabled;
    status(setting.error || `Brightness shortcuts ${setting.enabled ? 'on' : 'off'}.`, Boolean(setting.error));
  } catch (error) {
    $('#shortcuts').checked = !requested;
    status(String(error), true);
  } finally {
    $('#shortcuts').disabled = false;
  }
});
$('#brightness-keys').addEventListener('change', async () => {
  const requested = $('#brightness-keys').checked;
  $('#brightness-keys').disabled = true;
  try {
    const setting = await invoke('set_brightness_keys', { enabled: requested });
    info.keys = setting;
    render();
    status(setting.error || `The Mac's brightness keys ${setting.enabled ? 'on' : 'off'}.`, Boolean(setting.error));
  } catch (error) {
    $('#brightness-keys').checked = !requested;
    status(String(error), true);
  } finally {
    $('#brightness-keys').disabled = false;
  }
});
$('#login').addEventListener('change', async () => {
  const requested = $('#login').checked;
  $('#login').disabled = true;
  try {
    const setting = await invoke('set_login', { enabled: requested });
    $('#login').checked = setting.enabled;
    status(setting.error || `Start at login ${setting.enabled ? 'on' : 'off'}.`, Boolean(setting.error));
  } catch (error) {
    $('#login').checked = !requested;
    status(String(error), true);
  } finally {
    $('#login').disabled = false;
  }
});

async function initialise() {
  try {
    info = await invoke('settings_info');
    displays = await invoke('scan').catch(() => []);
    render();
  } catch (error) {
    status(String(error), true);
  } finally {
    document.body.dataset.ready = 'true';
  }
}

void initialise();
