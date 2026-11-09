#version 330
uniform sampler2D scene;
uniform sampler2D dep;      // monocular depth of the scene
uniform sampler2D emis;     // the lit panels alone: the only thing allowed to bloom
uniform vec2  shift;        // camera translation, in UV
uniform vec2  res;
uniform float glow;
uniform float grain;
uniform float time;         // 0..1 over one loop period, not seconds
uniform float flameScale;   // noise frequency, in cells across the frame
uniform float flameRise;    // how far the field advects over one period
uniform float flameAmp;     // how hard it modulates the panels
in  vec2 uv;
out vec4 frag;

#define PI 3.14159265

float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1,311.7)))*43758.5453); }
float vnoise(vec2 p){
    vec2 i = floor(p), f = fract(p);
    vec2 u = f*f*(3.0-2.0*f);
    return mix(mix(hash(i), hash(i+vec2(1,0)), u.x),
               mix(hash(i+vec2(0,1)), hash(i+vec2(1,1)), u.x), u.y);
}
float fbm(vec2 p){
    float a = 0.5, s = 0.0;
    for(int i=0;i<5;i++){ s += a*vnoise(p); p *= 2.03; a *= 0.5; }
    return s;
}

// A fire that returns exactly to its start.
//
// The loop contract (§2.3) measures the seam against an ordinary frame step,
// so a flame that merely looks turbulent is not enough -- it has to be
// periodic to the pixel. Advected noise is not: fbm(p - v*t) at t=T is a
// different field from t=0, and the cut shows as a jump every ten seconds,
// which is exactly the artefact an idle login screen gives you all night to
// notice.
//
// So two copies of the same advection, one period apart, crossfaded across
// the period. At w=0 the first is shown unadvected; at w=1 the second has
// advected by zero. They are the same field, so f(0) == f(T) exactly.
// smoothstep rather than a linear mix because its derivative vanishes at both
// ends: the velocity matches at the seam as well as the value, and a
// discontinuity in velocity is visible even when the value is continuous.
float loopfbm(vec2 p, float w){
    vec2 v = vec2(0.0, flameRise);
    float a = fbm(p - v*w);
    float b = fbm(p - v*(w - 1.0));
    return mix(a, b, smoothstep(0.0, 1.0, w));
}

void main(){
    vec2 px = vec2(uv.x, 1.0-uv.y) * res;

    // ---- the scene, with per-pixel parallax ------------------------------
    // Identical to nacre.frag: nothing is cut out, so nothing can be left
    // behind. 0.35 is the depth the camera pivots about.
    vec2 off = shift * (texture(dep, uv).r - 0.35);
    vec3 c = texture(scene, uv + off).rgb;

    // ---- the fire -------------------------------------------------------
    // The panels are painted, and they stay painted: this modulates their
    // BRIGHTNESS, it does not draw flames over them. A procedural flame drawn
    // on top would carry its own lighting into a painting that already has
    // some, which is the mistake nightshade-loop's README records about the
    // first charm -- a cut-out "read as a grey sliver".
    //
    // The mask is the emissive copy, so the modulation reaches exactly what
    // the scene says is alight and nothing else.
    vec3 e = texture(emis, uv + off).rgb;
    float m = clamp(max(max(e.r, e.g), e.b) * 3.0, 0.0, 1.0);

    // Sampled in aspect-corrected UV so the cells are square; the frame is
    // 16:9 and noise in raw UV comes out visibly stretched.
    vec2 fp = vec2(uv.x * res.x / res.y, uv.y) * flameScale;
    float f = loopfbm(fp, time);
    // Centred on zero so the mean brightness of the panels is unchanged and
    // only their variation is new. A flame that also raises the mean would
    // move the measured palette, and the palette is derived from this file.
    float drive = (f - 0.5) * 2.0;

    c *= 1.0 + m * drive * flameAmp;
    vec3 litE = e * (1.0 + drive * flameAmp);

    // ---- bloom, from the panels only -------------------------------------
    // Same eight-tap ring as nacre.frag, for the same reason: blooming the
    // whole frame blows out everything bright, and in this scene the stone is
    // the brightest thing after the fire.
    vec3 sum = vec3(0.0);
    for(int i=0;i<8;i++){
        float a = 2.0*PI*float(i)/8.0;
        vec2 d = vec2(cos(a), sin(a))*0.012;
        sum += texture(emis, uv + off + d).rgb * (1.0 + drive * flameAmp);
    }
    c += max(sum/8.0 - 0.10, 0.0) * glow;
    c += max(litE - 0.10, 0.0) * glow * 0.25;

    // ---- grain -----------------------------------------------------------
    // §4.1 asks for visible film grain, and it has to be static rather than
    // per-frame: animated grain is the one thing on an idle screen that reads
    // as noise rather than as film.
    c += (hash(px) - 0.5) * grain;

    frag = vec4(max(c, 0.0), 1.0);
}
