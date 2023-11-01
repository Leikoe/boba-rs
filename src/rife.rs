extern crate ggml_sys_bleedingedge as ggml;

use std::ffi::{c_int, CString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::slice::{from_raw_parts, from_raw_parts_mut};
use std::{env, ptr};

use anyhow::{bail, Result};
use ggml::*;

macro_rules! time {
        ($a:ident($($b:tt)*))=>{
            {
                    use std::time::Instant;

                    let start = Instant::now();
                    let result = $a($($b)*);
                    let end = start.elapsed();
                    eprintln!("[{}:{}] {} took {:?}", file!(), line!(), stringify!($a($($b)*)), end);
                    result
            }
        };
}

unsafe fn print_system_info() {
    println!(
        "AVX = {} | \
    AVX2 = {} | \
    AVX512 = {} | \
    AVX512_VBMI = {} | \
    AVX512_VNNI = {} | \
    FMA = {} | \
    NEON = {} | \
    ARM_FMA = {} | \
    F16C = {} | \
    FP16_VA = {} | \
    WASM_SIMD = {} | \
    BLAS = {} | \
    SSE3 = {} | \
    SSSE3 = {} | \
    VSX = {} |",
        ggml_cpu_has_avx(),
        ggml_cpu_has_avx2(),
        ggml_cpu_has_avx512(),
        ggml_cpu_has_avx512_vbmi(),
        ggml_cpu_has_avx512_vnni(),
        ggml_cpu_has_fma(),
        ggml_cpu_has_neon(),
        ggml_cpu_has_arm_fma(),
        ggml_cpu_has_f16c(),
        ggml_cpu_has_fp16_va(),
        ggml_cpu_has_wasm_simd(),
        ggml_cpu_has_blas(),
        ggml_cpu_has_sse3(),
        ggml_cpu_has_ssse3(),
        ggml_cpu_has_vsx()
    )
}

struct MnistModel {
    pub conv2d_1_kernel: *const ggml_tensor,
    pub conv2d_1_bias: *const ggml_tensor,
    pub conv2d_2_kernel: *const ggml_tensor,
    pub conv2d_2_bias: *const ggml_tensor,
    pub dense_weight: *const ggml_tensor,
    pub dense_bias: *const ggml_tensor,
    pub ctx: *const ggml_context,
}

unsafe fn mnist_model_load(model_path: String) -> Result<MnistModel> {
    let mut model_ctx: *mut ggml_context = ptr::null_mut();
    let params = gguf_init_params {
        no_alloc: false,
        ctx: &mut model_ctx,
    };

    let _ctx: *const gguf_context = gguf_init_from_file(CString::new(model_path)?.as_ptr(), params);
    if model_ctx.is_null() {
        bail!("gguf_init_from_file() failed");
    }

    Ok(MnistModel {
        conv2d_1_kernel: ggml_get_tensor(model_ctx, CString::new("kernel1")?.as_ptr()),
        conv2d_1_bias: ggml_get_tensor(model_ctx, CString::new("bias1")?.as_ptr()),
        conv2d_2_kernel: ggml_get_tensor(model_ctx, CString::new("kernel2")?.as_ptr()),
        conv2d_2_bias: ggml_get_tensor(model_ctx, CString::new("bias2")?.as_ptr()),
        dense_weight: ggml_get_tensor(model_ctx, CString::new("dense_w")?.as_ptr()),
        dense_bias: ggml_get_tensor(model_ctx, CString::new("dense_b")?.as_ptr()),
        ctx: model_ctx,
    })
}

unsafe fn mnist_eval(
    model: &MnistModel,
    n_threads: usize,
    digit: &[f32],
    fname_cgraph: Option<&str>,
) -> Result<i32> {
    let buf_size = 100000 * std::mem::size_of::<f32>() * 4;
    let mut buf = Vec::with_capacity(buf_size);

    let params = ggml_init_params {
        mem_size: buf_size,
        mem_buffer: buf.as_mut_ptr(),
        no_alloc: false,
    };

    let ctx0 = ggml_init(params);
    let gf = ggml_new_graph(ctx0);

    let mut input = ggml_new_tensor_4d(ctx0, ggml_type_GGML_TYPE_F32, 28, 28, 1, 1);
    let mut input_data = from_raw_parts_mut(((*input).data) as *mut f32, 28 * 28);
    input_data.copy_from_slice(digit);
    ggml_set_name(input, CString::new("input")?.as_ptr());

    let mut cur = ggml_conv_2d(
        ctx0,
        model.conv2d_1_kernel.cast_mut(),
        input,
        1,
        1,
        0,
        0,
        1,
        1,
    );
    cur = ggml_add(ctx0, cur, model.conv2d_1_bias.cast_mut());
    cur = ggml_relu(ctx0, cur);
    // Output shape after Conv2D: (26 26 32 1)
    cur = ggml_pool_2d(ctx0, cur, ggml_op_pool_GGML_OP_POOL_MAX, 2, 2, 2, 2, 0, 0);
    // Output shape after MaxPooling2D: (13 13 32 1)
    cur = ggml_conv_2d(
        ctx0,
        model.conv2d_2_kernel.cast_mut(),
        cur,
        1,
        1,
        0,
        0,
        1,
        1,
    );
    cur = ggml_add(ctx0, cur, model.conv2d_2_bias.cast_mut());
    cur = ggml_relu(ctx0, cur);

    // Output shape after Conv2D: (11 11 64 1)
    cur = ggml_pool_2d(ctx0, cur, ggml_op_pool_GGML_OP_POOL_MAX, 2, 2, 2, 2, 0, 0);
    // Output shape after MaxPooling2D: (5 5 64 1)
    cur = ggml_cont(ctx0, ggml_permute(ctx0, cur, 1, 2, 0, 3));
    // Output shape after permute: (64 5 5 1)
    cur = ggml_reshape_2d(ctx0, cur, 1600, 1);
    // Final Dense layer
    cur = ggml_add(
        ctx0,
        ggml_mul_mat(ctx0, model.dense_weight.cast_mut(), cur),
        model.dense_bias.cast_mut(),
    );

    let probs = ggml_soft_max(ctx0, cur);
    ggml_set_name(probs, CString::new("probs")?.as_ptr());

    ggml_build_forward_expand(gf, probs);
    ggml_graph_compute_with_ctx(ctx0, gf, n_threads as c_int);

    //ggml_graph_print(gf);
    //ggml_graph_dump_dot(gf, ptr::null(), CString::new("mnist-cnn.dot")?.as_ptr());

    if let Some(fname_cgraph) = fname_cgraph {
        // export the compute graph for later use
        // see the "mnist-cpu" example
        ggml_graph_export(gf, CString::new(fname_cgraph)?.as_ptr());

        println!("exported compute graph to '{}'", fname_cgraph);
    }

    // argmax of probs.data
    let prediction = from_raw_parts::<f32>(ggml_get_data_f32(cur), 10)
        .iter()
        .enumerate()
        .max_by(|(_, &a), (_, b)| a.total_cmp(b))
        .map(|(index, _)| index)
        .unwrap();

    ggml_free(ctx0);
    Ok(prediction as i32)
}

pub fn main() -> Result<()> {
    let (model_file, test_set_file) = match (env::args().nth(1), env::args().nth(2)) {
        (Some(mf), Some(tsf)) => (mf, tsf),
        _ => {
            bail!(format!(
                "Usage: {} models/mnist/mnist-cnn-model.gguf models/mnist/t10k-images.idx3-ubyte",
                env::args()
                    .next()
                    .expect("executable name should be defined")
            ));
        }
    };

    unsafe { print_system_info() };

    // load the model
    let model = unsafe { time!(mnist_model_load(model_file))? };

    // read a random digit from the test set
    let mut buf: [u8; 784] = [0; 784];
    let mut digit: [f32; 784] = [0f32; 784];
    {
        let mut fin = File::open(test_set_file)?;

        // seek to a random digit: 16-byte header + 28*28 * (random 0 - 10000)
        fin.seek(SeekFrom::Start(
            (16 + 784 * (rand::random::<usize>() % 10000)) as u64,
        ))?;
        fin.read_exact(&mut buf)?;
    }

    // render the digit in ASCII
    {
        for row in 0..28 {
            for col in 0..28 {
                eprint!("{} ", if buf[row * 28 + col] > 230 { '*' } else { '_' });
                digit[row * 28 + col] = buf[row * 28 + col] as f32;
            }

            eprintln!();
        }

        eprintln!();
    }

    let prediction = unsafe { time!(mnist_eval(&model, 1, &digit, None))? };
    println!("predicted digit is {}", prediction);

    unsafe { ggml_free(model.ctx.cast_mut()) };
    Ok(())
}
