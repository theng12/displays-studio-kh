const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (selector) => document.querySelector(selector);
const list = $('#displays');
const refreshButton = $('#refresh');
const shortcut = $('#shortcuts');
const news = $('#news');
const features = { 16: 'Brightness', 18: 'Contrast', 98: 'Volume' };
let busy = false;

function element(tag, text) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
}

function status(text, error = false) {
  $('#status').textContent = text;
  $('#status').toggleAttribute('data-error', error);
}

function showLevel(slider, output, percent) {
  slider.value = percent;
  slider.dataset.confirmed = percent;
  slider.setAttribute('aria-valuetext', `${percent}%`);
  output.value = `${percent}%`;
}

function control(display, level) {
  const row = element('label');
  row.className = 'level';
  const label = element('span', features[level.code]);
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
    busy = true;
    refreshButton.disabled = true;
    status(`Changing ${features[level.code].toLowerCase()}…`);
    try {
      const value = await invoke('set_level', { display: display.id, code: level.code, percent: slider.valueAsNumber });
      showLevel(slider, output, value.percent);
      status(`${display.name}: ${features[level.code].toLowerCase()} ${value.percent}%.`);
    } catch (error) {
      showLevel(slider, output, slider.dataset.confirmed);
      status(String(error), true);
    } finally {
      busy = false;
      refreshButton.disabled = false;
    }
  });
  row.append(label, slider, output);
  return row;
}

async function refresh() {
  if (busy) return;
  busy = true;
  refreshButton.disabled = true;
  list.setAttribute('aria-busy', 'true');
  status('Checking your displays…');
  try {
    const displays = await invoke('scan');
    list.replaceChildren(...displays.map((display) => {
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
    busy = false;
    refreshButton.disabled = false;
    list.removeAttribute('aria-busy');
  }
}

refreshButton.addEventListener('click', refresh);
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
      for (const [display, level] of changed) {
        const slider = [...list.querySelectorAll('input')].find((input) => input.dataset.display === display && Number(input.dataset.code) === level.code);
        if (slider) showLevel(slider, slider.nextElementSibling, level.percent);
      }
      status(message, failed);
    });
    const [version, setting, releaseNotes] = await invoke('panel_info');
    shortcut.checked = setting.enabled;
    shortcut.disabled = false;
    $('#version').textContent = `Displays Studio KH · v${version}`;
    $('#news-version').textContent = `v${version}`;
    await refresh();
    const notes = JSON.parse(releaseNotes).find((item) => item.version === version)?.items;
    if (!notes?.length) throw new Error(`Release notes are missing for v${version}.`);
    $('#notes').replaceChildren(...notes.map((item) => element('li', item)));
    if (setting.error) status(setting.error, true);
  } catch (error) {
    status(String(error), true);
  } finally {
    document.body.dataset.ready = 'true';
    window.dispatchEvent(new Event('displays-ready'));
  }
}

void initialise();
