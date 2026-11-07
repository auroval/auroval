#version 330
uniform sampler2D scene;
uniform sampler2D alb;      // skull albedo
uniform sampler2D salpha;   // skull coverage
uniform sampler2D snrm;     // skull normal map
uniform sampler2D dep;      // monocular depth of the scene
uniform sampler2D emis;     // the disc alone: the only thing allowed to bloom
uniform vec2  shift;        // camera translation, in UV
uniform vec2  res;
uniform vec2  halo;         // the disc: key light, in pixels
uniform vec2  beads[24];
uniform int   nbead;
uniform float brad;
uniform vec2  anchor;
uniform vec2  skullC;
uniform vec2  skullHalf;
uniform float tilt;
uniform float glow;
uniform float grain;
uniform float time;
in  vec2 uv;
out vec4 frag;

const float PI = 3.14159265359;

float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1,311.7)))*43758.5453); }
float vnoise(vec2 p){
    vec2 i = floor(p), f = fract(p);
    f = f*f*(3.0-2.0*f);
    return mix(mix(hash(i), hash(i+vec2(1,0)), f.x),
               mix(hash(i+vec2(0,1)), hash(i+vec2(1,1)), f.x), f.y);
}
float fbm(vec2 p){
    float s = 0.0, a = 0.5;
    for(int i=0;i<4;i++){ s += a*vnoise(p); p *= 2.07; a *= 0.5; }
    return s;
}

// Thin-film interference across stacked aragonite platelets. d is the film
// thickness in nanometres, cosT the cosine of the refracted angle. This is why
// nacre shifts colour with the angle instead of merely being shiny: the
// wavelengths that survive depend on the optical path through the film.
vec3 thinfilm(float d, float cosT){
    float opd = 2.0 * 1.56 * d * cosT;          // n(aragonite) ~= 1.56
    vec3  lam = vec3(650.0, 550.0, 450.0);
    return 0.5 + 0.5*cos(2.0*PI*opd/lam + PI);  // + PI: hard-boundary reflection
}

void main(){
    vec2 px = vec2(uv.x, 1.0-uv.y) * res;       // pixel coords, y down

    // ---- the scene, with the halo computed rather than baked -------------
    // Per-pixel parallax against a continuous depth map: nothing is cut out,
    // so nothing can be left behind.
    vec2 off = shift * (texture(dep, uv).r - 0.35);
    vec3 c = texture(scene, uv + off).rgb;
    // Bloom only from the disc. Blooming the whole frame also blew out her
    // kimono and her face, which are the brightest things in the painting and
    // have no business glowing: 14.3% of the frame clipped against 0.066% in
    // the painting itself, and 10.4% of that was on her rather than on the
    // disc. The emissive copy is the scene masked to its cyan, so white stays
    // white and only the halo spills.
    vec3 sum = vec3(0.0);
    for(int i=0;i<8;i++){
        float a = 2.0*PI*float(i)/8.0;
        sum += texture(emis, uv + off + vec2(cos(a), sin(a))*0.012).rgb;
    }
    c += max(sum/8.0 - 0.10, 0.0) * glow;

    vec3 V = vec3(0.0, 0.0, 1.0);

    // ---- skull: painted albedo, relit through its own normal map ---------
    {
        float s = sin(tilt), co = cos(tilt);
        vec2  q = px - skullC;
        q = vec2(q.x*co + q.y*s, -q.x*s + q.y*co);   // into the charm's frame
        vec2  st = q/skullHalf*0.5 + 0.5;
        if(all(greaterThanEqual(st, vec2(0.0))) && all(lessThanEqual(st, vec2(1.0)))){
            vec2 stt = vec2(st.x, 1.0-st.y);
            float a = texture(salpha, stt).r;
            if(a > 0.003){
                vec3 A = texture(alb, stt).rgb;
                vec3 N = normalize(texture(snrm, stt).rgb*vec3(1.0,-1.0,1.0));
                // rotate the normal back into screen space
                N = normalize(vec3(N.x*co - N.y*s, N.x*s + N.y*co, N.z));
                vec3 L = normalize(vec3(normalize(halo - px), 0.55));
                float dif  = max(dot(N,L), 0.0);
                float fres = pow(1.0 - max(dot(N,V),0.0), 3.5);
                float spec = pow(max(dot(reflect(-L,N), V), 0.0), 46.0);
                // Carved bone with a pearl surface: the albedo stays in charge,
                // the interference only skims the grazing angles.
                vec3 iri = thinfilm(430.0 + 110.0*fbm(st*4.0), max(dot(N,V),0.08));
                vec3 lit = A*(0.74 + 0.46*dif) + iri*fres*0.40 + vec3(spec)*0.55;
                c = mix(c, lit, clamp(a, 0.0, 1.0));
            }
        }
    }

    // ---- beads: analytic spheres of black mother-of-pearl ----------------
    for(int i=0;i<nbead;i++){
        if(i >= nbead) break;
        vec2 p = (px - beads[i]) / brad;
        float r2 = dot(p,p);
        if(r2 >= 1.0) continue;
        vec3 N = normalize(vec3(p.x, -p.y, sqrt(max(1.0-r2, 1e-4))));

        // Bump: irregular platelets. Without this a sphere reads as plastic,
        // because plastic is the only thing that is actually this smooth.
        // Low frequency on purpose. A pearl's surface is smooth; what is
        // irregular is the platelet stack UNDER it, so the bump has to be a
        // slow undulation. High-frequency noise here produced rainbow confetti,
        // which is what a cheap iridescent shader always looks like.
        vec2 bp = p*0.9 + vec2(float(i)*5.1, 0.0);
        float h  = fbm(bp);
        float hx = fbm(bp + vec2(0.16,0.0)) - h;
        float hy = fbm(bp + vec2(0.0,0.16)) - h;
        N = normalize(N + vec3(-hx, -hy, 0.0)*0.55);

        vec3 L = normalize(vec3(normalize(halo - beads[i]), 0.50));
        float cosT = max(dot(N,V), 0.05);
        float fres = pow(1.0 - cosT, 2.2);
        float dif  = max(dot(N,L), 0.0);
        float spec = pow(max(dot(reflect(-L,N), V), 0.0), 120.0);   // the pinpoint
        float over = pow(max(dot(reflect(-L,N), V), 0.0), 9.0);     // the overtone lobe

        // Film thickness varies over the bead, so the colour travels across it
        // instead of sitting in one place. A Tahitian pearl carries its colour
        // across the whole body, not only at the rim, so the interference is
        // weighted by the lighting rather than by Fresnel alone.
        // Narrow on purpose. A Tahitian pearl's overtone travels a short arc of the
        // spectrum --- green through aubergine --- not the whole of it. Letting
        // the thickness range freely gave a full rainbow, which is an oil slick.
        vec3 iri = thinfilm(352.0 + 55.0*h + 26.0*p.y - 18.0*p.x, cosT);
        iri = mix(vec3(dot(iri, vec3(0.2126,0.7152,0.0722))), iri, 0.72);

        // A little of the disc behind refracted through the surface layer:
        // black nacre is not opaque at its rim, and this is what stops the
        // beads reading as holes punched in the halo.
        vec3 refr = texture(scene, (px + N.xy*brad*2.2)/res*vec2(1.0,-1.0) + vec2(0.0,1.0) + off).rgb;

        vec3 body = vec3(0.022, 0.025, 0.034);              // black pearl
        vec3 col  = body
                  + body * dif * 3.0
                  + iri * (0.10 + 0.36*fres) * (0.25 + 0.75*dif)
                  + iri * over * 0.70
                  + vec3(spec) * 1.5
                  + refr * fres * 0.14;

        // Antialias in pixels, not in r-squared: squaring compresses the edge
        // into a band narrower than a pixel and the rim came out stippled.
        float cov = 1.0 - smoothstep(brad - 1.0, brad + 0.6, length(px - beads[i]));
        c = mix(c, col, cov);
    }

    c += (hash(uv*res + time*97.3) - 0.5) * grain;
    frag = vec4(clamp(c, 0.0, 1.0), 1.0);
}
