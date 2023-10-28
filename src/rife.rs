extern crate ggml_sys_bleedingedge as ggml;
use ggml::*;

#[derive(Debug)]
pub struct Error(String);


pub fn main() -> Result<(), Error> {
    //srand(time(NULL));
    GGMLSYS_VERSION;

    // if (argc != 3) {
    //     fprintf(stderr, "Usage: %s models/mnist/ggml-model-f32.bin models/mnist/t10k-images.idx3-ubyte\n", argv[0]);
    //     exit(0);
    // }
    //
    // uint8_t buf[784];
    // mnist_model model;
    // std::vector<float> digit;
    //
    // // load the model
    // {
    //     const int64_t t_start_us = ggml_time_us();
    //
    //     if (!mnist_model_load(argv[1], model)) {
    //         fprintf(stderr, "%s: failed to load model from '%s'\n", __func__, "models/ggml-model-f32.bin");
    //         return 1;
    //     }
    //
    //     const int64_t t_load_us = ggml_time_us() - t_start_us;
    //
    //     fprintf(stdout, "%s: loaded model in %8.2f ms\n", __func__, t_load_us / 1000.0f);
    // }
    //
    // // read a random digit from the test set
    // {
    //     std::ifstream fin(argv[2], std::ios::binary);
    //     if (!fin) {
    //         fprintf(stderr, "%s: failed to open '%s'\n", __func__, argv[2]);
    //         return 1;
    //     }
    //
    //     // seek to a random digit: 16-byte header + 28*28 * (random 0 - 10000)
    //     fin.seekg(16 + 784 * (rand() % 10000));
    //     fin.read((char *) &buf, sizeof(buf));
    // }
    //
    // // render the digit in ASCII
    // {
    //     digit.resize(sizeof(buf));
    //
    //     for (int row = 0; row < 28; row++) {
    //         for (int col = 0; col < 28; col++) {
    //             fprintf(stderr, "%c ", (float)buf[row*28 + col] > 230 ? '*' : '_');
    //             digit[row*28 + col] = ((float)buf[row*28 + col]);
    //         }
    //
    //         fprintf(stderr, "\n");
    //     }
    //
    //     fprintf(stderr, "\n");
    // }
    //
    // const int prediction = mnist_eval(model, 1, digit, "mnist.ggml");
    //
    // fprintf(stdout, "%s: predicted digit is %d\n", __func__, prediction);
    //
    // ggml_free(model.ctx);

    Ok(())
}