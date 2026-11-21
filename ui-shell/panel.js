const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (selector) => document.querySelector(selector);
const list = $('#displays');
const refreshButton = $('#refresh');
const shortcut = $('#shortcuts');
const login = $('#login');
const keys = $('#brightness-keys');
const intro = $('#intro');
const accessNote = $('#accessibility-note');
const news = $('#news');
const features = { 16: 'Brightness', 18: 'Contrast', 98: 'Volume' };
let busy = false;
let allSlider = null;
let keysTrusted = true;

function element(tag, text) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
}

function status(text, error = false) {
  $('#status').textContent = text;
  $('#status').toggleAttribute('data-error', error);
}

function setBusy(value) {
  busy = value;
  refreshButton.disabled = value;
  if (allSlider) allSlider.disabled = value;
}

function reconcile(changed) {
  for (const [display, level] of changed) {
    if (level.percent === null) continue;
    const slider = [...list.querySelectorAll('.level input')].find((input) => input.dataset.display === String(display) && Number(input.dataset.code) === level.code);
    if (slider) showLevel(slider, slider.nextElementSibling, level.percent);
  }
}

function updateAccessNote() {
  accessNote.hidden = !(keys.checked && !keysTrusted);
}

function allDisplaysSection() {
  const section = element('section');
  section.className = 'all-displays';
  const row = element('label');
  row.className = 'level';
  row.append(element('span', 'All displays'));
  allSlider = element('input');
  allSlider.type = 'range';
  allSlider.min = 0;
  allSlider.max = 100;
  allSlider.id = 'all-displays';
  allSlider.setAttribute('aria-label', 'Brightness for all displays');
  allSlider.setAttribute('aria-valuetext', `${allSlider.value}%`);
  allSlider.addEventListener('input', () => allSlider.setAttribute('aria-valuetext', `${allSlider.value}%`));
  allSlider.addEventListener('change', async () => {
    if (busy) return;
    setBusy(true);
    status('Changing brightness on all displays…');
    try {
      const result = await invoke('set_all', { code: 16, percent: allSlider.valueAsNumber });
      reconcile(result.changed);
      status(result.message || (result.failed ? 'Some displays did not confirm the change.' : ''), Boolean(result.failed));
    } catch (error) {
      status(String(error), true);
    } finally {
      setBusy(false);
    }
  });
  row.append(allSlider);
  section.append(row);
  return section;
}

function showLevel(slider, output, percent) {
  slider.value = percent;
  slider.dataset.confirmed = percent;
  slider.setAttribute('aria-valuetext', `${percent}%`);
  output.value = `${percent}%`;
}

function control(display, level) {
  const label = element('span', features[level.code]);
  if (level.percent === null) {
    // Nothing was read, so nothing is shown as a value and nothing can be dragged.
    const row = element('div');
    row.className = 'level unreported';
    row.append(label, element('span', 'Not reported'));
    return row;
  }
  const row = element('label');
  row.className = 'level';
  const slider = element('input');
  slider.type = 'range';
  slider.min = 0;
  slider.max = 100;
  slider.dataset.display = display.id;
  slider.dataset.code = level.code;
  slider.setAttribute('aria-label', `${features[level.code]} for ${display.name}`);
  const output = element('output');
  output.setAttribute('aria-hidden', 'true');
  showLevel(slider, output, level.percent);
  slider.addEventListener('input', () => {
    slider.setAttribute('aria-valuetext', `${slider.value}%`);
    output.value = `${slider.value}%`;
  });
  slider.addEventListener('change', async () => {
    if (busy) {
      showLevel(slider, output, slider.dataset.confirmed);
      return;
    }
    setBusy(true);
    status(`Changing ${features[level.code].toLowerCase()}…`);
    try {
      const value = await invoke('set_level', { display: display.id, code: level.code, percent: slider.valueAsNumber });
      showLevel(slider, output, value.percent);
      status(`${display.name}: ${features[level.code].toLowerCase()} ${value.percent}%.`);
    } catch (error) {
      showLevel(slider, output, slider.dataset.confirmed);
      status(String(error), true);
    } finally {
      setBusy(false);
    }
  });
  row.append(label, slider, output);
  return row;
}

async function refresh() {
  if (busy) return;
  setBusy(true);
  list.setAttribute('aria-busy', 'true');
  status('Checking your displays…');
  try {
    const displays = await invoke('scan');
    list.replaceChildren(allDisplaysSection(), ...displays.map((display) => {
      const section = element('section');
      section.append(element('h2', display.name));
      for (const level of display.levels) section.append(control(display, level));
      if (!display.levels.length) {
        section.append(element('p', 'No controls reported. Enable DDC/CI on the monitor if available, then refresh.'));
      }
      return section;
    }));
    status(displays.length ? '' : 'No external monitors found. Connect one, then refresh.');
  } catch (error) {
    list.replaceChildren();
    status(String(error), true);
  } finally {
    setBusy(false);
    list.removeAttribute('aria-busy');
  }
}

refreshButton.addEventListener('click', refresh);
try {
  if (!localStorage.getItem('displays-intro-seen')) intro.hidden = false;
} catch {
  intro.hidden = false;
}
$('#intro-dismiss').addEventListener('click', () => {
  intro.hidden = true;
  try { localStorage.setItem('displays-intro-seen', '1'); } catch { /* nothing to remember */ }
});
shortcut.addEventListener('change', async () => {
  const requested = shortcut.checked;
  shortcut.disabled = true;
  try {
    const setting = await invoke('set_shortcuts', { enabled: requested });
    shortcut.checked = setting.enabled;
    status(setting.error || `Brightness shortcuts ${setting.enabled ? 'on' : 'off'}.`, Boolean(setting.error));
  } catch (error) {
    shortcut.checked = !requested;
    status(String(error), true);
  } finally {
    shortcut.disabled = false;
  }
});
login.addEventListener('change', async () => {
  const requested = login.checked;
  login.disabled = true;
  try {
    const setting = await invoke('set_login', { enabled: requested });
    login.checked = setting.enabled;
    status(setting.error || `Start at login ${setting.enabled ? 'on' : 'off'}.`, Boolean(setting.error));
  } catch (error) {
    login.checked = !requested;
    status(String(error), true);
  } finally {
    login.disabled = false;
  }
});
keys.addEventListener('change', async () => {
  const requested = keys.checked;
  keys.disabled = true;
  try {
    const setting = await invoke('set_brightness_keys', { enabled: requested });
    keys.checked = setting.enabled;
    keysTrusted = setting.trusted;
    updateAccessNote();
    status(setting.error || `Brightness keys ${setting.enabled ? 'on' : 'off'}.`, Boolean(setting.error));
  } catch (error) {
    keys.checked = !requested;
    status(String(error), true);
  } finally {
    keys.disabled = false;
  }
});
$('#open-accessibility').addEventListener('click', () => {
  invoke('open_accessibility_settings').catch((error) => status(String(error), true));
});
$('#version').addEventListener('click', () => news.showModal());
news.addEventListener('click', (event) => {
  if (event.target !== news) return;
  const rect = news.getBoundingClientRect();
  if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) news.close();
});

document.addEventListener('keydown', (event) => {
  const close = event.metaKey && !event.shiftKey && !event.altKey && !event.ctrlKey && event.key.toLowerCase() === 'w';
  if (!close && (event.key !== 'Escape' || news.open)) return;
  event.preventDefault();
  invoke('close_panel').catch((error) => status(String(error), true));
});

async function initialise() {
  try {
    await listen('brightness-step', ({ payload }) => {
      if (payload.Err) { status(payload.Err, true); return; }
      const { changed, message, failed } = payload.Ok;
      reconcile(changed);
      status(message, failed);
    });
    const [version, shortcutsInfo, loginInfo, keysInfo, releaseNotes] = await invoke('panel_info');
    shortcut.checked = shortcutsInfo.enabled;
    shortcut.disabled = false;
    login.checked = loginInfo.enabled;
    login.disabled = false;
    keys.checked = keysInfo.enabled;
    keysTrusted = keysInfo.trusted;
    keys.disabled = false;
    updateAccessNote();
    $('#version').textContent = `Displays Studio KH · v${version}`;
    $('#news-version').textContent = `v${version}`;
    await refresh();
    const notes = JSON.parse(releaseNotes).find((item) => item.version === version)?.items;
    if (!notes?.length) throw new Error(`Release notes are missing for v${version}.`);
    $('#notes').replaceChildren(...notes.map((item) => element('li', item)));
    const errors = [shortcutsInfo.error, loginInfo.error, keysInfo.error].filter(Boolean);
    if (errors.length) status(errors.join(' '), true);
  } catch (error) {
    status(String(error), true);
  } finally {
    document.body.dataset.ready = 'true';
    window.dispatchEvent(new Event('displays-ready'));
  }
}

void initialise();
