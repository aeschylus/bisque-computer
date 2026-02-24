import * as THREE from 'three';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
import { ShaderPass } from 'three/addons/postprocessing/ShaderPass.js';

// ---------------------------------------------------------------------------
// Parameterised shader — all colors and mix values are uniforms
// ---------------------------------------------------------------------------

function makeShader() {
  return {
    uniforms: {
      tDiffuse: { value: null },
      uTime: { value: 0 },
      uResolution: { value: new THREE.Vector2(1, 1) },
      uMouse: { value: new THREE.Vector2(0.5, 0.5) },

      // Palette (vec3 as arrays)
      uBisque:      { value: new THREE.Color() },
      uBisqueWarm:  { value: new THREE.Color() },
      uBisqueDeep:  { value: new THREE.Color() },
      uHotPink:     { value: new THREE.Color() },
      uLobsterRed:  { value: new THREE.Color() },
      uDeepShadow:  { value: new THREE.Color() },
      uNeonCoral:   { value: new THREE.Color() },
      uCoolTeal:    { value: new THREE.Color() },
      uBurntSienna: { value: new THREE.Color() },

      // Mix strengths
      uOrangeMix:   { value: 0.65 },
      uPinkMix:     { value: 0.35 },
      uRedMix:      { value: 0.45 },
      uRedEdgeMix:  { value: 0.35 },
      uSiennaMix:   { value: 0.25 },
      uShadowMix:   { value: 0.55 },
      uVeinMix:     { value: 0.35 },
      uGlowMix:     { value: 0.30 },

      // Flow
      uFlowStrength:  { value: 0.12 },
      uWarpScale1:    { value: 3.0 },
      uWarpScale2:    { value: 2.0 },
      uWarpSpeed1:    { value: 0.7 },
      uWarpSpeed2:    { value: 0.45 },
      uTimeScale:     { value: 1.0 },

      // Effects
      uScanlineStr:   { value: 0.015 },
      uHalftoneStr:   { value: 0.30 },
      uGrainStr:      { value: 0.07 },
      uVignetteStr:   { value: 0.15 },
      uWarmPush:      { value: 0.30 },

      // Perf
      uFbmOctaves:    { value: 4 },
    },
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      precision mediump float;

      uniform float uTime;
      uniform vec2 uResolution;
      uniform vec2 uMouse;
      varying vec2 vUv;

      uniform vec3 uBisque, uBisqueWarm, uBisqueDeep;
      uniform vec3 uHotPink, uLobsterRed, uDeepShadow;
      uniform vec3 uNeonCoral, uCoolTeal, uBurntSienna;

      uniform float uOrangeMix, uPinkMix, uRedMix, uRedEdgeMix;
      uniform float uSiennaMix, uShadowMix, uVeinMix, uGlowMix;

      uniform float uFlowStrength, uWarpScale1, uWarpScale2;
      uniform float uWarpSpeed1, uWarpSpeed2, uTimeScale;

      uniform float uScanlineStr, uHalftoneStr, uGrainStr;
      uniform float uVignetteStr, uWarmPush;

      uniform int uFbmOctaves;

      float hash(vec2 p) {
        vec3 p3 = fract(vec3(p.xyx) * 0.1031);
        p3 += dot(p3, p3.yzx + 33.33);
        return fract((p3.x + p3.y) * p3.z);
      }

      float noise(vec2 p) {
        vec2 i = floor(p);
        vec2 f = fract(p);
        f = f * f * (3.0 - 2.0 * f);
        return mix(
          mix(hash(i), hash(i + vec2(1, 0)), f.x),
          mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), f.x),
          f.y
        );
      }

      float fbm(vec2 p) {
        float v = 0.0, a = 0.5;
        mat2 rot = mat2(0.8, 0.6, -0.6, 0.8);
        for (int i = 0; i < 6; i++) {
          if (i >= uFbmOctaves) break;
          v += a * noise(p);
          p = rot * p * 2.0;
          a *= 0.5;
        }
        return v;
      }

      float warpedFbm(vec2 p, float t) {
        vec2 q = vec2(
          fbm(p + 0.12 * t),
          fbm(p + vec2(5.2, 1.3) + 0.10 * t)
        );
        return fbm(p + 3.5 * q);
      }

      vec2 swirl(vec2 p, float t) {
        float a = fbm(p * 1.8 + t * 0.04) * 6.28;
        return vec2(cos(a), sin(a)) * uFlowStrength;
      }

      float halftone(vec2 uv, float angle) {
        float c = cos(angle), s = sin(angle);
        vec2 p = mat2(c, -s, s, c) * uv;
        return smoothstep(0.35, 0.3, length(fract(p) - 0.5));
      }

      float lineScreen(vec2 uv, float angle) {
        float c = cos(angle), s = sin(angle);
        float p = (mat2(c, -s, s, c) * uv).x;
        return smoothstep(0.4, 0.35, abs(fract(p) - 0.5));
      }

      void main() {
        vec2 uv = vUv;
        float aspect = uResolution.x / uResolution.y;
        vec2 p = (uv - 0.5) * vec2(aspect, 1.0);
        float t = uTime * uTimeScale;

        vec2 flow = swirl(p * 2.0, t);
        vec2 d = p + flow;

        vec2 mp = (uMouse - 0.5) * vec2(aspect, 1.0);
        float md = length(d - mp);
        d += normalize(d - mp + 0.001) * 0.02 / (md + 0.3);

        float w1 = warpedFbm(d * uWarpScale1, t * uWarpSpeed1);
        float w2 = warpedFbm(d * uWarpScale2 + 3.7, t * uWarpSpeed2);
        float detail = fbm(d * 5.0 + t * 0.12);

        // Base
        vec3 col = mix(uBisque, uBisqueWarm, 0.4);

        // Orange flow
        float orangeMask = smoothstep(0.30, 0.60, w1);
        col = mix(col, uBisqueDeep, orangeMask * uOrangeMix);

        // Hot pink
        float pinkMask = smoothstep(0.50, 0.75, w1);
        col = mix(col, uHotPink, pinkMask * uPinkMix);

        // Lobster red
        float redMask = smoothstep(0.40, 0.70, w2);
        float redEdge = smoothstep(0.38, 0.42, w2) * (1.0 - smoothstep(0.68, 0.72, w2));
        col = mix(col, uLobsterRed, redMask * uRedMix);
        col = mix(col, uNeonCoral, redEdge * uRedEdgeMix);

        // Sienna
        float siennaMask = smoothstep(0.35, 0.55, w1 * 0.6 + w2 * 0.4);
        col = mix(col, uBurntSienna, siennaMask * uSiennaMix);

        // Shadow
        float shadowMask = smoothstep(0.50, 0.80, w1 * w2);
        col = mix(col, uDeepShadow, shadowMask * uShadowMix);

        // Teal veins
        float veinNoise = fbm(d * 7.0 + vec2(t * 0.08, t * 0.06));
        float veins = smoothstep(0.48, 0.50, veinNoise) * smoothstep(0.52, 0.50, veinNoise);
        veins *= 3.0;
        col = mix(col, uCoolTeal, veins * uVeinMix);

        // Glow
        float glowMask = smoothstep(0.55, 0.75, detail);
        col = mix(col, uBisqueWarm * 1.1, glowMask * uGlowMix);

        // Scanlines
        float scan = sin(uv.y * uResolution.y * 1.5 + t * 2.0) * 0.5 + 0.5;
        col += vec3(uScanlineStr) * scan * scan * scan;

        // Halftone
        float ht = halftone(uv * uResolution.xy / 3.0, 0.523);
        col = mix(col, col * 0.88 + uBisqueDeep * 0.12, ht * orangeMask * uHalftoneStr);

        float ls = lineScreen(uv * uResolution.xy / 4.0, 1.047);
        col = mix(col, col * 0.92 + uLobsterRed * 0.08, ls * redMask * uHalftoneStr * 0.8);

        // Stipple
        float stipple = step(0.82, hash(floor(d * 25.0) + floor(t * 0.5)));
        col = mix(col, uCoolTeal * 0.7, stipple * shadowMask * 0.2);

        // Grain
        float grain = hash(uv * uResolution.xy + fract(t * 43.17)) * uGrainStr * 2.0 - uGrainStr;
        col += grain;

        // Vignette
        float vig = 1.0 - smoothstep(0.4, 1.4, length(p * 1.2));
        col *= (1.0 - uVignetteStr) + uVignetteStr * vig;

        // Bloom
        float bloom = smoothstep(0.6, 0.9, w1) * smoothstep(0.5, 0.8, w2);
        col += uNeonCoral * bloom * 0.08;
        col += uCoolTeal * veins * 0.1;

        // Warm push
        col = mix(col, col * vec3(1.04, 0.97, 0.88), uWarmPush);

        col = clamp(col, 0.0, 1.0);
        col = pow(col, vec3(0.95));
        gl_FragColor = vec4(col, 1.0);
      }
    `,
  };
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

const PRESETS = {
  'v1': {
    label: 'V1 Original',
    colors: {
      uBisque:      '#ffe4c4', uBisqueWarm:  '#ffe4c4', uBisqueDeep:  '#d9a633',
      uHotPink:     '#ff1493', uLobsterRed:  '#c41e3a', uDeepShadow:  '#1a0a2e',
      uNeonCoral:   '#ff6b6b', uCoolTeal:    '#1affd5', uBurntSienna: '#b84c1e',
    },
    mix: {
      uOrangeMix: 0.30, uPinkMix: 0.70, uRedMix: 0.60, uRedEdgeMix: 0.40,
      uSiennaMix: 0.0, uShadowMix: 0.50, uVeinMix: 0.50, uGlowMix: 0.30,
    },
    flow: {
      uFlowStrength: 0.15, uWarpScale1: 3.0, uWarpScale2: 2.0,
      uWarpSpeed1: 0.8, uWarpSpeed2: 0.5, uTimeScale: 1.0,
    },
    fx: {
      uScanlineStr: 0.02, uHalftoneStr: 0.40, uGrainStr: 0.08,
      uVignetteStr: 0.15, uWarmPush: 0.0,
    },
    perf: { uFbmOctaves: 6 },
  },
  'v2': {
    label: 'V2 Bisque',
    colors: {
      uBisque:      '#ffddba', uBisqueWarm:  '#fac78c', uBisqueDeep:  '#eb9e40',
      uHotPink:     '#f23385', uLobsterRed:  '#d12638', uDeepShadow:  '#2e0f14',
      uNeonCoral:   '#ff7359', uCoolTeal:    '#1ad9c0', uBurntSienna: '#b84c1e',
    },
    mix: {
      uOrangeMix: 0.65, uPinkMix: 0.35, uRedMix: 0.45, uRedEdgeMix: 0.35,
      uSiennaMix: 0.25, uShadowMix: 0.55, uVeinMix: 0.35, uGlowMix: 0.30,
    },
    flow: {
      uFlowStrength: 0.12, uWarpScale1: 3.0, uWarpScale2: 2.0,
      uWarpSpeed1: 0.7, uWarpSpeed2: 0.45, uTimeScale: 1.0,
    },
    fx: {
      uScanlineStr: 0.015, uHalftoneStr: 0.30, uGrainStr: 0.07,
      uVignetteStr: 0.15, uWarmPush: 0.30,
    },
    perf: { uFbmOctaves: 4 },
  },
  'cyber': {
    label: 'Cyberpunk',
    colors: {
      uBisque:      '#1a1a2e', uBisqueWarm:  '#16213e', uBisqueDeep:  '#0f3460',
      uHotPink:     '#e94560', uLobsterRed:  '#ff006e', uDeepShadow:  '#0a0a0a',
      uNeonCoral:   '#ff4444', uCoolTeal:    '#00ffcc', uBurntSienna: '#533483',
    },
    mix: {
      uOrangeMix: 0.50, uPinkMix: 0.60, uRedMix: 0.50, uRedEdgeMix: 0.40,
      uSiennaMix: 0.30, uShadowMix: 0.70, uVeinMix: 0.60, uGlowMix: 0.20,
    },
    flow: {
      uFlowStrength: 0.18, uWarpScale1: 4.0, uWarpScale2: 2.5,
      uWarpSpeed1: 1.0, uWarpSpeed2: 0.7, uTimeScale: 1.2,
    },
    fx: {
      uScanlineStr: 0.04, uHalftoneStr: 0.20, uGrainStr: 0.05,
      uVignetteStr: 0.25, uWarmPush: 0.0,
    },
    perf: { uFbmOctaves: 4 },
  },
  'lava': {
    label: 'Lava',
    colors: {
      uBisque:      '#ff6600', uBisqueWarm:  '#ff4400', uBisqueDeep:  '#cc2200',
      uHotPink:     '#ff0044', uLobsterRed:  '#990000', uDeepShadow:  '#1a0000',
      uNeonCoral:   '#ff8844', uCoolTeal:    '#ffcc00', uBurntSienna: '#661100',
    },
    mix: {
      uOrangeMix: 0.70, uPinkMix: 0.50, uRedMix: 0.60, uRedEdgeMix: 0.40,
      uSiennaMix: 0.35, uShadowMix: 0.65, uVeinMix: 0.40, uGlowMix: 0.40,
    },
    flow: {
      uFlowStrength: 0.20, uWarpScale1: 2.5, uWarpScale2: 1.8,
      uWarpSpeed1: 0.5, uWarpSpeed2: 0.3, uTimeScale: 0.7,
    },
    fx: {
      uScanlineStr: 0.01, uHalftoneStr: 0.15, uGrainStr: 0.04,
      uVignetteStr: 0.20, uWarmPush: 0.10,
    },
    perf: { uFbmOctaves: 5 },
  },
};

// ---------------------------------------------------------------------------
// Create two renderers side by side
// ---------------------------------------------------------------------------

const CTRL_WIDTH = 320;

function paneWidth() {
  return Math.floor((window.innerWidth - CTRL_WIDTH - 2) / 2);
}

function createPane(container) {
  const w = paneWidth();
  const h = window.innerHeight;
  const renderer = new THREE.WebGLRenderer({ preserveDrawingBuffer: true });
  renderer.setPixelRatio(1.0);
  renderer.setSize(w, h);
  container.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  scene.add(new THREE.Mesh(
    new THREE.PlaneGeometry(2, 2),
    new THREE.MeshBasicMaterial({ color: 0x000000 })
  ));

  const composer = new EffectComposer(renderer);
  composer.addPass(new RenderPass(scene, camera));
  const shader = makeShader();
  const pass = new ShaderPass(shader);
  composer.addPass(pass);

  pass.uniforms.uResolution.value.set(w, h);

  return { renderer, composer, pass, uniforms: pass.uniforms };
}

const leftPane = createPane(document.getElementById('pane-left'));
const rightPane = createPane(document.getElementById('pane-right'));

// ---------------------------------------------------------------------------
// Apply preset to uniforms
// ---------------------------------------------------------------------------

function applyPreset(uniforms, preset) {
  for (const [k, hex] of Object.entries(preset.colors)) {
    uniforms[k].value.set(hex);
  }
  for (const [k, v] of Object.entries(preset.mix)) {
    uniforms[k].value = v;
  }
  for (const [k, v] of Object.entries(preset.flow)) {
    uniforms[k].value = v;
  }
  for (const [k, v] of Object.entries(preset.fx)) {
    uniforms[k].value = v;
  }
  for (const [k, v] of Object.entries(preset.perf)) {
    uniforms[k].value = v;
  }
}

// Left = V1, Right = V2
applyPreset(leftPane.uniforms, PRESETS.v1);
applyPreset(rightPane.uniforms, PRESETS.v2);

// ---------------------------------------------------------------------------
// Control panel — modifies RIGHT pane only
// ---------------------------------------------------------------------------

const activeUniforms = rightPane.uniforms;

function hexFromColor(c) {
  return '#' + c.getHexString();
}

function buildColorControls() {
  const container = document.getElementById('color-controls');
  const colorKeys = [
    ['uBisque', 'Bisque Base'], ['uBisqueWarm', 'Bisque Warm'], ['uBisqueDeep', 'Bisque Deep'],
    ['uHotPink', 'Hot Pink'], ['uLobsterRed', 'Lobster Red'], ['uDeepShadow', 'Deep Shadow'],
    ['uNeonCoral', 'Neon Coral'], ['uCoolTeal', 'Cool Teal'], ['uBurntSienna', 'Burnt Sienna'],
  ];
  for (const [key, label] of colorKeys) {
    const row = document.createElement('div');
    row.className = 'ctrl-row';
    const lbl = document.createElement('label');
    lbl.textContent = label;
    const input = document.createElement('input');
    input.type = 'color';
    input.value = hexFromColor(activeUniforms[key].value);
    input.dataset.key = key;
    input.addEventListener('input', (e) => {
      activeUniforms[e.target.dataset.key].value.set(e.target.value);
    });
    const hexLabel = document.createElement('span');
    hexLabel.className = 'color-label';
    hexLabel.textContent = input.value;
    input.addEventListener('input', () => { hexLabel.textContent = input.value; });
    row.append(lbl, input, hexLabel);
    container.appendChild(row);
  }
}

function buildSliderGroup(containerId, specs) {
  const container = document.getElementById(containerId);
  for (const [key, label, min, max, step] of specs) {
    const row = document.createElement('div');
    row.className = 'ctrl-row';
    const lbl = document.createElement('label');
    lbl.textContent = label;
    const input = document.createElement('input');
    input.type = 'range';
    input.min = min; input.max = max; input.step = step;
    input.value = activeUniforms[key].value;
    input.dataset.key = key;
    const val = document.createElement('span');
    val.className = 'val';
    val.textContent = Number(input.value).toFixed(2);
    input.addEventListener('input', (e) => {
      const v = parseFloat(e.target.value);
      activeUniforms[e.target.dataset.key].value = v;
      val.textContent = v.toFixed(2);
    });
    row.append(lbl, input, val);
    container.appendChild(row);
  }
}

buildColorControls();

buildSliderGroup('mix-controls', [
  ['uOrangeMix',  'Orange',      0, 1, 0.01],
  ['uPinkMix',    'Pink',        0, 1, 0.01],
  ['uRedMix',     'Red',         0, 1, 0.01],
  ['uRedEdgeMix', 'Red Edge',    0, 1, 0.01],
  ['uSiennaMix',  'Sienna',      0, 1, 0.01],
  ['uShadowMix',  'Shadow',      0, 1, 0.01],
  ['uVeinMix',    'Teal Veins',  0, 1, 0.01],
  ['uGlowMix',    'Glow',        0, 1, 0.01],
]);

buildSliderGroup('flow-controls', [
  ['uFlowStrength', 'Flow Str',    0, 0.5, 0.01],
  ['uWarpScale1',   'Warp Scale 1', 0.5, 8, 0.1],
  ['uWarpScale2',   'Warp Scale 2', 0.5, 8, 0.1],
  ['uWarpSpeed1',   'Warp Speed 1', 0, 2, 0.05],
  ['uWarpSpeed2',   'Warp Speed 2', 0, 2, 0.05],
  ['uTimeScale',    'Time Scale',   0, 3, 0.05],
]);

buildSliderGroup('fx-controls', [
  ['uScanlineStr', 'Scanlines',  0, 0.1, 0.001],
  ['uHalftoneStr', 'Halftone',   0, 1, 0.01],
  ['uGrainStr',    'Grain',      0, 0.2, 0.005],
  ['uVignetteStr', 'Vignette',   0, 0.5, 0.01],
  ['uWarmPush',    'Warm Push',  0, 1, 0.01],
]);

buildSliderGroup('perf-controls', [
  ['uFbmOctaves', 'FBM Octaves', 1, 6, 1],
]);

// Preset buttons apply to RIGHT pane and update all sliders/pickers
function refreshControls() {
  document.querySelectorAll('#controls input').forEach(el => {
    const key = el.dataset.key;
    if (!key) return;
    const u = activeUniforms[key];
    if (el.type === 'color') {
      el.value = hexFromColor(u.value);
      const hexLabel = el.nextElementSibling;
      if (hexLabel) hexLabel.textContent = el.value;
    } else if (el.type === 'range') {
      el.value = u.value;
      const valSpan = el.nextElementSibling;
      if (valSpan) valSpan.textContent = Number(u.value).toFixed(2);
    }
  });
}

document.getElementById('preset-v1').addEventListener('click', () => {
  applyPreset(activeUniforms, PRESETS.v1);
  refreshControls();
});
document.getElementById('preset-v2').addEventListener('click', () => {
  applyPreset(activeUniforms, PRESETS.v2);
  refreshControls();
});
document.getElementById('preset-cyber').addEventListener('click', () => {
  applyPreset(activeUniforms, PRESETS.cyber);
  refreshControls();
});
document.getElementById('preset-lava').addEventListener('click', () => {
  applyPreset(activeUniforms, PRESETS.lava);
  refreshControls();
});

// ---------------------------------------------------------------------------
// Perf HUD
// ---------------------------------------------------------------------------

const hud = document.getElementById('hud');
let frameCount = 0, fpsAccum = 0, lastFpsUpdate = performance.now();

// ---------------------------------------------------------------------------
// Animation
// ---------------------------------------------------------------------------

const clock = new THREE.Clock();
let lastTime = performance.now();

function animate() {
  requestAnimationFrame(animate);

  const now = performance.now();
  const delta = now - lastTime;
  lastTime = now;
  frameCount++;
  fpsAccum += delta;

  if (now - lastFpsUpdate > 500) {
    const fps = Math.round(frameCount / (fpsAccum / 1000));
    const ms = (fpsAccum / frameCount).toFixed(1);
    hud.textContent = `${fps} fps | ${ms}ms/frame | 2x panes`;
    frameCount = 0; fpsAccum = 0; lastFpsUpdate = now;
  }

  const t = clock.getElapsedTime();

  leftPane.uniforms.uTime.value = t;
  rightPane.uniforms.uTime.value = t;

  leftPane.composer.render();
  rightPane.composer.render();
}
animate();

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

function handleResize() {
  const w = paneWidth();
  const h = window.innerHeight;
  for (const pane of [leftPane, rightPane]) {
    pane.renderer.setSize(w, h);
    pane.composer.setSize(w, h);
    pane.uniforms.uResolution.value.set(w, h);
  }
}

window.addEventListener('resize', handleResize);

window.addEventListener('mousemove', (e) => {
  const mx = e.clientX / window.innerWidth;
  const my = 1.0 - e.clientY / window.innerHeight;
  leftPane.uniforms.uMouse.value.set(mx, my);
  rightPane.uniforms.uMouse.value.set(mx, my);
});

// Click on a pane to save its PNG
for (const pane of [leftPane, rightPane]) {
  pane.renderer.domElement.addEventListener('click', () => {
    const link = document.createElement('a');
    link.download = `bisque-texture-${Date.now()}.png`;
    link.href = pane.renderer.domElement.toDataURL('image/png');
    link.click();
  });
}
