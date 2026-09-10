#version 330 core
in vec2 position;
out vec4 outputColor;
uniform sampler2D image;
uniform sampler2D foreground;
uniform vec4 tint, secondary, bounds, clip, uv;
uniform float radius, angle, fadeTop, fadeBottom, dimming;
uniform int kind;
// Keep values aligned with Rust SpriteKind.
const int RoundedRect = 0;
const int AlphaMask = 1;
const int TextWithForeground = 3;
const int Progress = 4;
void main() {
    vec2 p = position;
    if (any(lessThan(p,clip.xy)) || any(greaterThanEqual(p,clip.xy+clip.zw))) discard;
    vec2 center = bounds.xy+bounds.zw*0.5;
    vec2 delta = p-center;
    float c = cos(angle), s = sin(angle);
    vec2 local = mat2(c,-s,s,c)*delta + bounds.zw*0.5;
    vec4 value;
    if (kind == Progress) {
        float left = clamp(local.x+0.5,0.0,1.0);
        float right = clamp(bounds.z-local.x+0.5,0.0,1.0);
        float track = clamp(uv.y*0.5-abs(local.y-bounds.w*0.5)+0.5,0.0,1.0)*left*right;
        float fill = clamp(bounds.z*uv.x-local.x+0.5,0.0,1.0)*left
            *clamp(bounds.w*0.5-abs(local.y-bounds.w*0.5)+0.5,0.0,1.0);
        value = vec4(tint.rgb*fill+secondary.rgb*track*(1.0-fill),fill+track*(1.0-fill))*tint.a;
    } else if (kind == RoundedRect) {
        vec2 d = abs(local-bounds.zw*0.5) - bounds.zw*0.5 + radius;
        float distance = length(max(d,0.0))+min(max(d.x,d.y),0.0)-radius;
        float amount = clamp(0.5-distance,0.0,1.0);
        value = vec4(tint.rgb*tint.a,tint.a)*amount;
    } else {
        vec2 coord = local/bounds.zw;
        if (any(lessThan(coord,vec2(0))) || any(greaterThanEqual(coord,vec2(1)))) discard;
        vec4 texel = texture(image,uv.xy+coord*uv.zw);
        if (kind == AlphaMask) value = vec4(tint.rgb*tint.a,tint.a)*texel.r;
        else if (kind == TextWithForeground) {
            vec3 ink = texture(foreground,uv.xy+coord*uv.zw).rgb;
            value = vec4(texel.rgb+ink*tint.rgb,texel.a)*tint.a;
        } else value = texel*tint.a;
    }
    float fade = 1.0;
    if (fadeTop > 0.0) fade *= clamp((p.y-clip.y)/fadeTop,0.0,1.0);
    if (fadeBottom > 0.0) fade *= clamp((clip.y+clip.w-p.y)/fadeBottom,0.0,1.0);
    value.rgb *= 1.0-dimming;
    outputColor = value*fade;
}
