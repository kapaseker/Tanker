// 设计稿 CSS 背景与九枚 SVG 方块，坐标以逻辑像素和原始 viewBox 为单位。
import bevy_ui_render::ui_vertex_output::UiVertexOutput;
@group(1) @binding(0) var<uniform> viewport: vec4<f32>;
@group(1) @binding(1) var<uniform> cubes: vec4<f32>;

fn over(base: vec3<f32>, color: vec3<f32>, alpha: f32) -> vec3<f32> {
    return mix(base, color, clamp(alpha, 0.0, 1.0));
}
fn edge(a: vec2<f32>, b: vec2<f32>, p: vec2<f32>) -> f32 {
    let v=b-a; let q=p-a; return (v.x*q.y-v.y*q.x)/length(v);
}
fn face(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, d: vec2<f32>, aa: f32) -> f32 {
    return smoothstep(-aa,aa,min(min(edge(a,b,p),edge(b,c,p)),min(edge(c,d,p),edge(d,a,p))));
}
fn ease(t: f32) -> f32 {
    var lo=0.0; var hi=1.0;
    for(var i=0;i<12;i++) {
        let u=(lo+hi)*0.5; let x=3.0*(1.0-u)*(1.0-u)*u*0.42+3.0*(1.0-u)*u*u*0.58+u*u*u;
        if x<t {lo=u;} else {hi=u;}
    }
    let u=(lo+hi)*0.5; return 3.0*(1.0-u)*u*u+u*u*u;
}
fn drift(delay: f32) -> f32 {
    let t=fract((viewport.z+delay)/7.0);
    // 与 HTML 的 -18px 一致，保持原来的周期、相位和局部缩放。
    if t<0.5 {return -18.0*ease(t*2.0);} return -18.0*(1.0-ease((t-0.5)*2.0));
}
fn linear(color: vec3<f32>) -> vec3<f32> {
    return select(pow((color+0.055)/1.055,vec3(2.4)),color/12.92,color<=vec3(0.04045));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size=viewport.xy;
    let pixel=in.uv*size;
    var color=vec3(15.0,14.0,12.0)/255.0;
    let sand=length((pixel-size*vec2(0.82,1.08))/(size*vec2(0.90,0.70)));
    color=over(color,vec3(150.0,116.0,82.0)/255.0,0.42*clamp(1.0-sand/0.62,0.0,1.0));
    let mint=length((pixel-size*vec2(0.88,0.02))/(size*vec2(0.55,0.45)));
    color=over(color,vec3(200.0,219.0,177.0)/255.0,0.10*clamp(1.0-mint/0.70,0.0,1.0));
    // 反解 perspective(520px) rotateX(66deg)，保持与 CSS 的地面网格坐标一致。
    let dy=pixel.y-size.y*1.14;
    let denom=cos(1.1519173)+dy*sin(1.1519173)/520.0;
    if denom>0.0 {
        let y=dy/denom;
        let x=(pixel.x-size.x*0.5)*(1.0-y*sin(1.1519173)/520.0)+size.x*0.72;
        if y<=0.0 && y>=-size.y*0.56 {
            let grid=min(abs(fract(x/62.0)*62.0),abs(fract((y+size.y*0.56)/62.0)*62.0));
            let mask=clamp((0.82+y/(size.y*0.56))/0.70,0.0,1.0);
            color=over(color,vec3(200.0,219.0,177.0)/255.0,0.10*0.60*mask*(1.0-smoothstep(0.5,1.5,grid)));
        }
    }
    color=over(color,vec3(0.0),select(0.0,0.16*0.22,fract(pixel.y/3.0)>2.0/3.0));
    let vignette=length((pixel-size*vec2(0.50,0.45))/(size*vec2(1.20,0.90)));
    color=over(color,vec3(0.0),0.62*clamp((vignette-0.45)/0.55,0.0,1.0));

    if cubes.z>0.0 {
        let blocks=array<vec4<f32>,9>(vec4(330.0,210.0,1.15,1.0),vec4(440.0,300.0,0.75,1.0),
            vec4(250.0,330.0,0.90,1.0),vec4(160.0,220.0,0.45,0.55),vec4(400.0,150.0,0.40,0.60),
            vec4(486.0,190.0,0.50,0.50),vec4(530.0,330.0,0.30,0.35),vec4(120.0,330.0,0.35,0.40),vec4(300.0,118.0,0.30,0.40));
        let delays=array<f32,9>(2.4,4.1,0.8,5.3,3.2,1.6,6.0,2.0,4.7);
        let palette=array<u32,9>(0u,1u,2u,1u,0u,3u,0u,2u,1u);
        let tops=array<vec3<f32>,4>(vec3(226.0,237.0,207.0),vec3(216.0,219.0,216.0),vec3(139.0,118.0,96.0),vec3(58.0,56.0,48.0));
        let lefts=array<vec3<f32>,4>(vec3(183.0,203.0,159.0),vec3(176.0,181.0,178.0),vec3(109.0,91.0,73.0),vec3(44.0,42.0,36.0));
        let rights=array<vec3<f32>,4>(vec3(145.0,169.0,122.0),vec3(134.0,140.0,137.0),vec3(82.0,68.0,55.0),vec3(33.0,31.0,26.0));
        let scale=cubes.z/470.0;
        // 阴影先合成于所有方块后方，对应 SVG 上的整体 drop-shadow。
        var shadow=0.0;
        for(var i=0u;i<9u;i++) {
            let b=blocks[i];
            let center=cubes.xy+(b.xy-vec2(90.0,95.0)+vec2(0.0,(36.0+drift(delays[i]))*b.z))*scale+vec2(0.0,30.0);
            let radius=vec2(34.0,36.0)*b.z*scale;
            let q=max(abs(pixel-center)-radius,vec2(0.0));
            let blur=20.0;
            let a=exp(-dot(q,q)/(2.0*blur*blur))*b.w*0.45*0.35;
            shadow=1.0-(1.0-shadow)*(1.0-a);
        }
        color=over(color,vec3(0.0),shadow);
        for(var i=0u;i<9u;i++) {
            let b=blocks[i]; let s=b.z*scale;
            let origin=cubes.xy+(b.xy-vec2(90.0,95.0))*scale;
            let p=(pixel-origin)/s-vec2(0.0,drift(delays[i]));
            let aa=0.6/s; let a=b.w*0.95;
            color=over(color,tops[palette[i]]/255.0,a*face(p,vec2(0.0,0.0),vec2(34.0,17.0),vec2(0.0,34.0),vec2(-34.0,17.0),aa));
            color=over(color,lefts[palette[i]]/255.0,a*face(p,vec2(-34.0,17.0),vec2(0.0,34.0),vec2(0.0,72.0),vec2(-34.0,55.0),aa));
            color=over(color,rights[palette[i]]/255.0,a*face(p,vec2(0.0,34.0),vec2(34.0,17.0),vec2(34.0,55.0),vec2(0.0,72.0),aa));
        }
    }
    return vec4(linear(color),1.0);
}
