#version 330 core
in vec2 position;
out vec4 color;
uniform vec2 viewport, physicalViewport;
uniform isampler2D noise;
uniform sampler2D art0, art1, art2;
uniform isampler2D gradient0, gradient1, gradient2;
struct Graphic {
    vec4 canvas;
    vec4 bounds, plateBounds;
    vec4 background, border, plate, quiet, muted;
    uvec3 steps;
    float borderWidth, shadowRadius, shadowY, shadowAlpha, weight;
    bool hasArtwork, hasGradient;
};
uniform Graphic graphics[3];

float coverage(vec2 p, vec4 r) {
    vec2 edge = min(p-r.xy, r.xy+r.zw-p);
    vec2 amount = clamp(edge + vec2(0.5), 0.0, 1.0);
    return amount.x * amount.y;
}
vec3 erfApprox(vec3 x) {
    vec3 s = sign(x), a = abs(x);
    vec3 t = 1.0 / (1.0 + 0.3275911 * a);
    return s * (1.0 - (((((1.061405429*t - 1.453152027)*t)
        + 1.421413741)*t - 0.284496736)*t + 0.254829592)*t*exp(-a*a));
}
float shadow(vec2 p, vec4 r, float radius) {
    float sigma = max(radius * 0.5, 0.01);
    // Beyond six standard deviations the erf approximation rounds to one
    // in float precision, so this product is exactly zero.
    if (any(lessThan(p, r.xy-vec2(6.0*sigma)))
        || any(greaterThan(p, r.xy+r.zw+vec2(6.0*sigma)))) return 0.0;
    vec2 low = (p-r.xy) / (sigma * 1.41421356237);
    vec2 high = (p-r.xy-r.zw) / (sigma * 1.41421356237);
    vec2 c = (erfApprox(vec3(low, 0)).xy - erfApprox(vec3(high, 0)).xy) * 0.5;
    return c.x*c.y;
}
vec3 gradientColor(Graphic g, isampler2D lookup) {
    uvec2 pixel = uvec2((position * viewport - g.canvas.xy) * physicalViewport / viewport);
    uint index = (g.steps.x + pixel.x*g.steps.y + pixel.y*g.steps.z) >> 16;
    ivec4 entry = texelFetch(lookup, ivec2(index & 255u, index >> 8), 0);
    ivec3 value = entry.rgb;
    if (entry.a != 0) value += texelFetch(noise, ivec2(pixel & uvec2(127u)), 0).rgb;
    // Negative rounded channels clamp to zero, so clamp before rounding and
    // shift nonnegative values instead of dividing signed values per channel.
    ivec3 rounded = (max(value, ivec3(0)) + 8192) >> 14;
    return vec3(min(rounded, ivec3(255))) / 255.0;
}
vec3 drawGraphic(Graphic g, sampler2D artwork, isampler2D lookup) {
#ifdef SHARED_GEOMETRY
    g.canvas = graphics[0].canvas;
    g.bounds = graphics[0].bounds;
    g.plateBounds = graphics[0].plateBounds;
    g.borderWidth = graphics[0].borderWidth;
    g.shadowRadius = graphics[0].shadowRadius;
    g.shadowY = graphics[0].shadowY;
    g.hasArtwork = true;
#endif
    vec2 p = position * viewport - g.canvas.xy;
    if (g.canvas.z > 0.0 && (any(lessThan(p,vec2(0))) || any(greaterThanEqual(p,g.canvas.zw)))) return vec3(0);
    vec4 inside = g.bounds + vec4(g.borderWidth, g.borderWidth, -2*g.borderWidth, -2*g.borderWidth);
    float inner = coverage(p, inside);
    vec4 texel = vec4(0);
    if (g.hasArtwork && inner > 0.0) {
        texel = texture(artwork, (p-inside.xy)/inside.zw);
        // Opaque artwork completely covers the backdrop and decorations.
        if (inner == 1.0 && texel.a == 1.0) return texel.rgb;
    }
    vec3 result = g.hasGradient ? gradientColor(g, lookup) : g.background.rgb;
    if (g.bounds.z <= 0.0 || g.bounds.w <= 0.0) return result;
    vec4 shadowBounds = g.bounds + vec4(0, g.shadowY, 0, 0);
    result = mix(result, g.background.rgb, shadow(p, shadowBounds, g.shadowRadius)*g.shadowAlpha);
    result = mix(result, g.plate.rgb, coverage(p, g.plateBounds)*g.plate.a);
    float outer = coverage(p, g.bounds);
    vec3 field = g.quiet.rgb;
    if (!g.hasArtwork) {
        // The quiet artwork field retains its authored 142-degree three-stop
        // treatment and inset border independently of the supplied-art path.
        vec2 q = (p-g.bounds.xy)/g.bounds.zw;
        vec2 direction = vec2(0.6156614753, 0.7880107536);
        float t = dot(q, direction) / (direction.x+direction.y);
        vec3 first = mix(g.quiet.rgb, g.muted.rgb, 0.09);
        field = t < 0.52 ? mix(first, g.quiet.rgb, clamp(t/0.52,0.0,1.0))
                         : mix(g.quiet.rgb, g.background.rgb, clamp((t-0.52)/0.48,0.0,1.0));
        float inner = coverage(p, g.bounds+vec4(24,24,-48,-48));
        field = mix(field, g.background.rgb, (1.0-inner)*0.16);
    }
    result = mix(result, field, outer);
    result = mix(result, g.border.rgb, max(0.0,outer-inner)*g.border.a);
    if (g.hasArtwork) {
        result = mix(result, texel.rgb, inner*texel.a);
    }
    return result;
}
void main() {
    vec3 result = vec3(0);
    if (graphics[0].weight > 0.0) result += drawGraphic(graphics[0],art0,gradient0)*graphics[0].weight;
    if (graphics[1].weight > 0.0) result += drawGraphic(graphics[1],art1,gradient1)*graphics[1].weight;
    if (graphics[2].weight > 0.0) result += drawGraphic(graphics[2],art2,gradient2)*graphics[2].weight;
    color = vec4(result,1);
}
