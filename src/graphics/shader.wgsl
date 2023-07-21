struct VertexInput {
    // @builtin(vertex_index) i: u32,
    @location(0) col: vec4<f32>,
    @location(1) pos: vec2<f32>,
};

struct FragmentInput {
    @builtin(position) pos: vec4<f32>,
    @location(0) col: vec4<f32>,
};

struct Push {
    mvp: mat4x4<f32>,
};

var<push_constant> push: Push;

@vertex
fn vs_main(vin: VertexInput) -> FragmentInput {
    var fin: FragmentInput;
    fin.pos = push.mvp * vec4<f32>(vin.pos, 0.0, 1.0);
    fin.col = vin.col;
    return fin;
}

@fragment
fn fs_main(fin: FragmentInput) -> @location(0) vec4<f32> {
    return fin.col;
}
