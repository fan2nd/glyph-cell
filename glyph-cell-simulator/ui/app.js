const invoke = window.__TAURI__?.core?.invoke;

const alignmentOptions = [
  ["topLeft", "Top left"],
  ["topCenter", "Top center"],
  ["topRight", "Top right"],
  ["middleLeft", "Middle left"],
  ["center", "Center"],
  ["middleRight", "Middle right"],
  ["bottomLeft", "Bottom left"],
  ["bottomCenter", "Bottom center"],
  ["bottomRight", "Bottom right"],
];

const state = {
  settings: null,
  fonts: [],
  renderTimer: 0,
  renderSeq: 0,
};

const numericFields = [
  "collectionIndex",
  "fontSize",
  "asciiWidth",
  "spacing",
  "lineSpacing",
  "zoom",
];

const $ = (id) => document.getElementById(id);

window.addEventListener("DOMContentLoaded", async () => {
  fillAlignmentOptions();
  bindControls();

  if (!invoke) {
    setStatus("Tauri API unavailable");
    return;
  }

  try {
    const initial = await invoke("get_initial_state");
    state.settings = initial.settings;
    state.fonts = initial.systemFonts;
    populateSystemFonts();
    syncControls();
    applyRender(initial.render);
    setStatus("Ready");
  } catch (error) {
    setStatus("Startup failed");
    showFatal(String(error));
  }
});

function fillAlignmentOptions() {
  const select = $("alignment");
  select.replaceChildren();
  for (const [value, label] of alignmentOptions) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    select.append(option);
  }
}

function bindControls() {
  $("fontSource").addEventListener("change", async (event) => {
    state.settings.fontSource = event.target.value;
    syncFontVisibility();
    if (state.settings.fontSource === "custom" && !state.settings.customFontPath) {
      await chooseFontFile();
    } else {
      scheduleRender();
    }
  });

  $("systemFont").addEventListener("change", (event) => {
    state.settings.selectedSystemFont = Number(event.target.value);
    scheduleRender();
  });

  $("refreshFonts").addEventListener("click", refreshSystemFonts);
  $("chooseFont").addEventListener("click", chooseFontFile);

  $("text").addEventListener("input", (event) => {
    state.settings.text = event.target.value;
    scheduleRender();
  });

  $("glyphYOffsets").addEventListener("input", (event) => {
    state.settings.glyphYOffsets = event.target.value;
    scheduleRender();
  });

  for (const id of ["layoutMode", "flow", "alignment"]) {
    $(id).addEventListener("change", (event) => {
      state.settings[id] = event.target.value;
      if (id === "layoutMode") {
        syncLayoutControls();
      }
      scheduleRender();
    });
  }

  $("glyphColor").addEventListener("input", (event) => {
    state.settings.glyphColor = hexToColor(event.target.value);
    scheduleRender();
  });

  $("debugCell").addEventListener("change", (event) => {
    state.settings.debugOverlays.cell = event.target.checked;
    scheduleRender();
  });

  $("debugGlyph").addEventListener("change", (event) => {
    state.settings.debugOverlays.glyph = event.target.checked;
    scheduleRender();
  });

  for (const field of numericFields) {
    bindNumberPair(field);
  }

  $("toggleCode").addEventListener("click", toggleCodePanel);
  $("copyCode").addEventListener("click", copyExampleCode);
}

function bindNumberPair(field) {
  const range = $(`${field}Range`);
  const number = $(`${field}Number`);
  const update = (raw) => {
    const value = parseClamped(raw, number);
    state.settings[field] = value;

    if (field === "fontSize") {
      syncAsciiWidthBounds();
      state.settings.asciiWidth = parseClamped(state.settings.asciiWidth, $("asciiWidthNumber"));
      syncNumberPair("asciiWidth");
    }

    syncNumberPair(field);
    scheduleRender();
  };

  range.addEventListener("input", (event) => update(event.target.value));
  number.addEventListener("change", (event) => update(event.target.value));
}

function parseClamped(raw, input) {
  const step = Number(input.step || 1);
  const min = Number(input.min);
  const max = Number(input.max);
  let value = step % 1 === 0 ? Number.parseInt(raw, 10) : Number.parseFloat(raw);

  if (!Number.isFinite(value)) {
    value = min;
  }

  value = Math.min(max, Math.max(min, value));
  if (step % 1 !== 0) {
    value = Math.round(value / step) * step;
    return Number(value.toFixed(2));
  }
  return Math.round(value);
}

function syncControls() {
  $("fontSource").value = state.settings.fontSource;
  $("systemFont").value = String(state.settings.selectedSystemFont);
  $("customPath").value = state.settings.customFontPath || "";
  $("text").value = state.settings.text;
  $("glyphYOffsets").value = state.settings.glyphYOffsets;
  $("layoutMode").value = state.settings.layoutMode;
  $("flow").value = state.settings.flow;
  $("alignment").value = state.settings.alignment;
  $("glyphColor").value = colorToHex(state.settings.glyphColor);
  $("debugCell").checked = state.settings.debugOverlays.cell;
  $("debugGlyph").checked = state.settings.debugOverlays.glyph;
  syncAsciiWidthBounds();
  state.settings.asciiWidth = parseClamped(state.settings.asciiWidth, $("asciiWidthNumber"));
  syncLayoutControls();

  for (const field of numericFields) {
    syncNumberPair(field);
  }

  syncFontVisibility();
}

function syncNumberPair(field) {
  const value = state.settings[field];
  $(`${field}Range`).value = String(value);
  $(`${field}Number`).value = String(value);
}

function syncAsciiWidthBounds() {
  const { min, max } = asciiWidthBounds();
  for (const input of [$("asciiWidthRange"), $("asciiWidthNumber")]) {
    input.min = String(min);
    input.max = String(max);
  }
}

function asciiWidthBounds() {
  const fontSize = Number(state.settings?.fontSize || 4);
  return {
    min: Math.ceil(fontSize / 2),
    max: fontSize,
  };
}

function syncLayoutControls() {
  const asciiWidthActive = state.settings.layoutMode === "monospace";
  $("asciiWidthField").hidden = !asciiWidthActive;
  $("asciiWidthRange").disabled = !asciiWidthActive;
  $("asciiWidthNumber").disabled = !asciiWidthActive;
}

function syncFontVisibility() {
  const usingSystem = state.settings.fontSource === "system";
  $("systemFontField").hidden = !usingSystem;
  $("refreshFonts").hidden = !usingSystem;
  $("chooseFont").hidden = usingSystem;
  $("customPath").hidden = usingSystem;
}

function populateSystemFonts() {
  const select = $("systemFont");
  select.replaceChildren();

  if (state.fonts.length === 0) {
    const option = document.createElement("option");
    option.value = "0";
    option.textContent = "No system fonts";
    select.append(option);
    return;
  }

  state.fonts.forEach((font, index) => {
    const option = document.createElement("option");
    option.value = String(index);
    option.textContent = font.label;
    option.title = font.path;
    select.append(option);
  });
}

async function refreshSystemFonts() {
  setStatus("Refreshing");
  try {
    state.fonts = await invoke("refresh_system_fonts");
    state.settings.selectedSystemFont = Math.min(
      state.settings.selectedSystemFont,
      Math.max(0, state.fonts.length - 1),
    );
    populateSystemFonts();
    syncControls();
    scheduleRender(0);
  } catch (error) {
    setStatus("Refresh failed");
    showFatal(String(error));
  }
}

async function chooseFontFile() {
  setStatus("Choosing");
  try {
    const selected = await invoke("choose_font_file", {
      current: state.settings.customFontPath || null,
    });

    if (selected) {
      state.settings.customFontPath = selected;
      state.settings.fontSource = "custom";
      syncControls();
      scheduleRender(0);
    } else {
      setStatus("Ready");
    }
  } catch (error) {
    setStatus("Choose failed");
    showFatal(String(error));
  }
}

function scheduleRender(delay = 60) {
  window.clearTimeout(state.renderTimer);
  state.renderTimer = window.setTimeout(renderNow, delay);
}

async function renderNow() {
  const seq = ++state.renderSeq;
  setStatus("Rendering");

  try {
    const render = await invoke("render_preview", {
      settings: state.settings,
    });

    if (seq !== state.renderSeq) {
      return;
    }

    applyRender(render);
    setStatus("Ready");
  } catch (error) {
    if (seq === state.renderSeq) {
      setStatus("Render failed");
      showFatal(String(error));
    }
  }
}

function applyRender(render) {
  drawPreview(render);
  $("measurement").textContent =
    `Measured text: ${render.measurement.width} x ${render.measurement.height} px | ` +
    `Canvas: ${render.width} x ${render.height} px`;
  $("exampleCode").value = render.exampleCode;
  $("fontPath").textContent = render.font.path ? `Path: ${render.font.path}` : "";

  const fontStatus = $("fontStatus");
  fontStatus.classList.toggle("error", Boolean(render.font.error));
  fontStatus.textContent = render.font.error
    ? render.font.error
    : `Loaded glyphs: ${render.font.loadedGlyphs} | Index: ${render.font.index}`;

  const warnings = [];
  if (render.font.missingChars) {
    warnings.push(`Missing glyphs in selected font: ${render.font.missingChars}`);
  }
  if (render.font.clippedChars) {
    warnings.push(`Vertically clipped glyphs: ${render.font.clippedChars}`);
  }
  $("fontWarnings").replaceChildren(
    ...warnings.map((text) => {
      const line = document.createElement("div");
      line.textContent = text;
      return line;
    })
  );
}

function drawPreview(render) {
  const canvas = $("previewCanvas");
  const stage = $("canvasStage");
  const grid = $("gridOverlay");
  const context = canvas.getContext("2d", { alpha: true });
  const zoom = Number(state.settings.zoom || 1);
  const displayWidth = Math.round(render.width * zoom);
  const displayHeight = Math.round(render.height * zoom);

  canvas.width = render.width;
  canvas.height = render.height;
  context.clearRect(0, 0, render.width, render.height);
  context.putImageData(
    new ImageData(new Uint8ClampedArray(render.rgba), render.width, render.height),
    0,
    0,
  );

  stage.style.width = `${displayWidth}px`;
  stage.style.height = `${displayHeight}px`;
  canvas.style.width = `${displayWidth}px`;
  canvas.style.height = `${displayHeight}px`;
  grid.style.setProperty("--grid-step", `${zoom}px`);
  grid.style.setProperty("--grid-color", gridColor(zoom));
}

function gridColor(zoom) {
  const alpha = Math.round(Math.min(128, Math.max(28, 24 + zoom * 8))) / 255;
  return `rgba(138, 151, 161, ${alpha.toFixed(3)})`;
}

function toggleCodePanel() {
  const collapsed = document.body.classList.toggle("code-collapsed");
  $("toggleCode").textContent = collapsed ? "<" : ">";
  $("toggleCode").title = collapsed ? "Show example code" : "Hide example code";
}

async function copyExampleCode() {
  const code = $("exampleCode");
  try {
    await navigator.clipboard.writeText(code.value);
    setStatus("Copied");
    window.setTimeout(() => setStatus("Ready"), 900);
  } catch {
    code.focus();
    code.select();
    document.execCommand("copy");
    setStatus("Copied");
    window.setTimeout(() => setStatus("Ready"), 900);
  }
}

function colorToHex(color) {
  return `#${toHex(color.r)}${toHex(color.g)}${toHex(color.b)}`;
}

function hexToColor(hex) {
  const value = hex.replace("#", "");
  return {
    r: Number.parseInt(value.slice(0, 2), 16),
    g: Number.parseInt(value.slice(2, 4), 16),
    b: Number.parseInt(value.slice(4, 6), 16),
  };
}

function toHex(value) {
  return Number(value).toString(16).padStart(2, "0");
}

function setStatus(text) {
  $("statusText").textContent = text;
}

function showFatal(text) {
  const fontStatus = $("fontStatus");
  fontStatus.classList.add("error");
  fontStatus.textContent = text;
}
