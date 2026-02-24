import * as THREE from 'three';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
import { ShaderPass } from 'three/addons/postprocessing/ShaderPass.js';

// ---------------------------------------------------------------------------
// Performance: render at reduced resolution, scale up
// ---------------------------------------------------------------------------

const DPR_CAP = 1.0; // Cap pixel ratio — single biggest perf win
const renderer = new THREE.WebGLRenderer({ preserveDrawingBuffer: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio, DPR_CAP));
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.domElement.style.imageRendering = 'auto'; // bilinear upscale
document.body.appendChild(renderer.domElement);

const scene = new THREE.Scene();
const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

const quad = new THREE.Mesh(
  new THREE.PlaneGeometry(2, 2),
  new THREE.MeshBasicMaterial({ color: 0x000000 })
);
scene.add(quad);

// ---------------------------------------------------------------------------
// Performance HUD
// ---------------------------------------------------------------------------

const hud = document.getElementById('info');
let frameCount = 0;
let fpsAccum = 0;
let lastFpsUpdate = performance.now();
let displayFps = 0;
let displayMs = 0;
let gpuTimings = [];

// GPU timing via EXT_disjoint_timer_query (if available)
const gl = renderer.getContext();
const timerExt = gl.getExtension('EXT_disjoint_timer_query_webgl2')
  || gl.getExtension('EXT_disjoint_timer_query');
let gpuQuery = null;
let gpuQueryPending = false;

function beginGpuTimer() {
  if (!timerExt) return;
  if (gpuQueryPending) return;
  gpuQuery = gl.createQuery();
  gl.beginQuery(timerExt.TIME_ELAPSED_EXT || gl.TIME_ELAPSED, gpuQuery);
}

function endGpuTimer() {
  if (!timerExt || !gpuQuery) return;
  gl.endQuery(timerExt.TIME_ELAPSED_EXT || gl.TIME_ELAPSED);
  gpuQueryPending = true;
}

function pollGpuTimer() {
  if (!timerExt || !gpuQuery || !gpuQueryPending) return;
  const available = gl.getQueryParameter(gpuQuery, gl.QUERY_RESULT_AVAILABLE);
  const disjoint = gl.getParameter(timerExt.GPU_DISJOINT_EXT || gl.GPU_DISJOINT);
  if (available && !disjoint) {
    const ns = gl.getQueryParameter(gpuQuery, gl.QUERY_RESULT);
    gpuTimings.push(ns / 1e6); // ms
    if (gpuTimings.length > 60) gpuTimings.shift();
    gl.deleteQuery(gpuQuery);
    gpuQuery = null;
    gpuQueryPending = false;
  }
}

function avgGpuMs() {
  if (gpuTimings.length === 0) return 0;
  return gpuTimings.reduce((a, b) => a + b, 0) / gpuTimings.length;
}

// ---------------------------------------------------------------------------
// Shader: Flowing Bisque Cyberpunk Texture
//
// PERF optimizations vs v1:
//   - FBM reduced from 6 → 4 octaves
//   - warpedFbm: 5 FBM calls → 3 (single warp layer, not double)
//   - Curl field: 3 FBM calls → removed, replaced with cheap sin/cos swirl
//   - Voronoi: removed entirely (9-tap loop was expensive)
//   - Registration offset warpedFbm: removed (was a full extra warp)
//   - lineScreen/halftone: kept (cheap trig)
//   - Total FBM evals per pixel: ~17 → ~7
// ---------------------------------------------------------------------------

const BisqueFlowShader = {
  uniforms: {
    tDiffuse: { value: null },
    uTime: { value: 0 },
    uResolution: { value: new THREE.Vector2(window.innerWidth, window.innerHeight) },
    uMouse: { value: new THREE.Vector2(0.5, 0.5) },
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

    // --- Palette (shifted warmer / more bisque-orange) ---
    const vec3 BISQUE       = vec3(1.0, 0.87, 0.72);
    const vec3 BISQUE_WARM  = vec3(0.98, 0.78, 0.55);   // warm amber bisque
    const vec3 BISQUE_DEEP  = vec3(0.92, 0.62, 0.25);   // deep orange bisque
    const vec3 HOT_PINK     = vec3(0.95, 0.20, 0.52);   // toned down slightly
    const vec3 LOBSTER_RED  = vec3(0.82, 0.15, 0.22);
    const vec3 DEEP_SHADOW  = vec3(0.18, 0.06, 0.08);   // warm dark, not indigo
    const vec3 NEON_CORAL   = vec3(1.0, 0.45, 0.35);
    const vec3 COOL_TEAL    = vec3(0.10, 0.85, 0.75);   // kept as accent only
    const vec3 BURNT_SIENNA = vec3(0.72, 0.30, 0.12);

    // --- Noise (4-octave FBM) ---

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

    // 4-octave FBM — sweet spot for quality vs cost
    float fbm(vec2 p) {
      float v = 0.0, a = 0.5;
      mat2 rot = mat2(0.8, 0.6, -0.6, 0.8);
      for (int i = 0; i < 4; i++) {
        v += a * noise(p);
        p = rot * p * 2.0;
        a *= 0.5;
      }
      return v;
    }

    // Single-layer domain warp (v1 had double — halves FBM calls)
    float warpedFbm(vec2 p, float t) {
      vec2 q = vec2(
        fbm(p + 0.12 * t),
        fbm(p + vec2(5.2, 1.3) + 0.10 * t)
      );
      return fbm(p + 3.5 * q);
    }

    // Cheap swirl field — replaces expensive curl (was 3x FBM)
    vec2 swirl(vec2 p, float t) {
      float a = fbm(p * 1.8 + t * 0.04) * 6.28;
      return vec2(cos(a), sin(a)) * 0.12;
    }

    // Halftone dot pattern (cheap)
    float halftone(vec2 uv, float angle) {
      float c = cos(angle), s = sin(angle);
      vec2 p = mat2(c, -s, s, c) * uv;
      return smoothstep(0.35, 0.3, length(fract(p) - 0.5));
    }

    // Line screen (cheap)
    float lineScreen(vec2 uv, float angle) {
      float c = cos(angle), s = sin(angle);
      float p = (mat2(c, -s, s, c) * uv).x;
      return smoothstep(0.4, 0.35, abs(fract(p) - 0.5));
    }

    void main() {
      vec2 uv = vUv;
      float aspect = uResolution.x / uResolution.y;
      vec2 p = (uv - 0.5) * vec2(aspect, 1.0);
      float t = uTime;

      // --- Flow distortion (cheap swirl replaces curl) ---
      vec2 flow = swirl(p * 2.0, t);
      vec2 d = p + flow;

      // Mouse influence
      vec2 mp = (uMouse - 0.5) * vec2(aspect, 1.0);
      float md = length(d - mp);
      d += normalize(d - mp + 0.001) * 0.02 / (md + 0.3);

      // --- Domain warping (2 layers instead of 3) ---
      float w1 = warpedFbm(d * 3.0, t * 0.7);
      float w2 = warpedFbm(d * 2.0 + 3.7, t * 0.45);

      // Cheap extra detail layer — single FBM, no warp
      float detail = fbm(d * 5.0 + t * 0.12);

      // --- Color composition (bisque-orange dominant) ---

      // Base: warm bisque
      vec3 col = mix(BISQUE, BISQUE_WARM, 0.4);

      // Layer 1: orange flow rivers (was pink-dominant, now bisque-orange)
      float orangeMask = smoothstep(0.30, 0.60, w1);
      col = mix(col, BISQUE_DEEP, orangeMask * 0.65);

      // Layer 2: hot pink accents — reduced presence
      float pinkMask = smoothstep(0.50, 0.75, w1);
      col = mix(col, HOT_PINK, pinkMask * 0.35);

      // Layer 3: lobster red pools
      float redMask = smoothstep(0.40, 0.70, w2);
      float redEdge = smoothstep(0.38, 0.42, w2) * (1.0 - smoothstep(0.68, 0.72, w2));
      col = mix(col, LOBSTER_RED, redMask * 0.45);
      col = mix(col, NEON_CORAL, redEdge * 0.35);

      // Layer 4: burnt sienna mid-tones
      float siennaMask = smoothstep(0.35, 0.55, w1 * 0.6 + w2 * 0.4);
      col = mix(col, BURNT_SIENNA, siennaMask * 0.25);

      // Layer 5: warm shadows (not indigo — warm dark)
      float shadowMask = smoothstep(0.50, 0.80, w1 * w2);
      col = mix(col, DEEP_SHADOW, shadowMask * 0.55);

      // Layer 6: teal veins — cyberpunk accent, minimal
      float veinNoise = fbm(d * 7.0 + vec2(t * 0.08, t * 0.06));
      float veins = smoothstep(0.48, 0.50, veinNoise) * smoothstep(0.52, 0.50, veinNoise);
      veins *= 3.0;
      col = mix(col, COOL_TEAL, veins * 0.35);

      // Layer 7: bisque glow at peaks
      float glowMask = smoothstep(0.55, 0.75, detail);
      col = mix(col, BISQUE_WARM * 1.1, glowMask * 0.3);

      // --- Scanlines ---
      float scan = sin(uv.y * uResolution.y * 1.5 + t * 2.0) * 0.5 + 0.5;
      col += vec3(0.015) * scan * scan * scan;

      // --- Risograph halftone (cheap) ---
      float ht = halftone(uv * uResolution.xy / 3.0, 0.523);
      col = mix(col, col * 0.88 + BISQUE_DEEP * 0.12, ht * orangeMask * 0.3);

      float ls = lineScreen(uv * uResolution.xy / 4.0, 1.047);
      col = mix(col, col * 0.92 + LOBSTER_RED * 0.08, ls * redMask * 0.25);

      // --- Stipple in shadows (cheap hash, no voronoi) ---
      float stipple = step(0.82, hash(floor(d * 25.0) + floor(t * 0.5)));
      col = mix(col, COOL_TEAL * 0.7, stipple * shadowMask * 0.2);

      // --- Paper grain ---
      float grain = hash(uv * uResolution.xy + fract(t * 43.17)) * 0.07 - 0.035;
      col += grain;

      // --- Vignette ---
      float vig = 1.0 - smoothstep(0.4, 1.4, length(p * 1.2));
      col *= 0.85 + 0.15 * vig;

      // --- Bloom glow ---
      float bloom = smoothstep(0.6, 0.9, w1) * smoothstep(0.5, 0.8, w2);
      col += NEON_CORAL * bloom * 0.08;
      col += COOL_TEAL * veins * 0.1;

      // Final warm push — bias everything slightly toward orange
      col = mix(col, col * vec3(1.04, 0.97, 0.88), 0.3);

      col = clamp(col, 0.0, 1.0);
      col = pow(col, vec3(0.95));

      gl_FragColor = vec4(col, 1.0);
    }
  `,
};

// ---------------------------------------------------------------------------
// Postprocessing pipeline
// ---------------------------------------------------------------------------

const composer = new EffectComposer(renderer);
composer.addPass(new RenderPass(scene, camera));

const bisquePass = new ShaderPass(BisqueFlowShader);
composer.addPass(bisquePass);

// ---------------------------------------------------------------------------
// Animation loop with perf instrumentation
// ---------------------------------------------------------------------------

const clock = new THREE.Clock();
let lastTime = performance.now();

function animate() {
  requestAnimationFrame(animate);

  const now = performance.now();
  const frameDelta = now - lastTime;
  lastTime = now;

  frameCount++;
  fpsAccum += frameDelta;

  // Update HUD every 500ms
  if (now - lastFpsUpdate > 500) {
    displayFps = Math.round(frameCount / (fpsAccum / 1000));
    displayMs = (fpsAccum / frameCount).toFixed(1);
    frameCount = 0;
    fpsAccum = 0;
    lastFpsUpdate = now;

    const gpu = timerExt ? ` | gpu ${avgGpuMs().toFixed(1)}ms` : '';
    const res = renderer.getDrawingBufferSize(new THREE.Vector2());
    hud.textContent =
      `${displayFps} fps | ${displayMs}ms${gpu} | ${res.x}x${res.y} | click to save PNG`;
  }

  pollGpuTimer();
  beginGpuTimer();

  bisquePass.uniforms.uTime.value = clock.getElapsedTime();
  composer.render();

  endGpuTimer();
}
animate();

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

window.addEventListener('resize', () => {
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, DPR_CAP));
  renderer.setSize(window.innerWidth, window.innerHeight);
  composer.setSize(window.innerWidth, window.innerHeight);
  bisquePass.uniforms.uResolution.value.set(window.innerWidth, window.innerHeight);
});

window.addEventListener('mousemove', (e) => {
  bisquePass.uniforms.uMouse.value.set(
    e.clientX / window.innerWidth,
    1.0 - e.clientY / window.innerHeight
  );
});

renderer.domElement.addEventListener('click', () => {
  const link = document.createElement('a');
  link.download = `bisque-texture-${Date.now()}.png`;
  link.href = renderer.domElement.toDataURL('image/png');
  link.click();
});
