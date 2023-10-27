use std::path::Path;
use ffmpeg::{format, codec, Error, decoder};
use ffmpeg::media::Type;
use ffmpeg::sys::avcodec_alloc_context3;

struct CodingContext<T> {
        format_ctx: format::context::Input,
        coder: T,
        video_stream_index: i32,
}

fn open_input<P: AsRef<Path>>(file: &P, decoder_name: Option<&str>) -> Result<CodingContext<codec::decoder::Video>, ffmpeg::Error> {
        let format_ctx = format::input(file)?;
        let video_stream = format_ctx.streams().best(Type::Video)
            .expect("failed to find video stream");
        let video_stream_index = video_stream.id();
        let codec_ctx = match decoder_name {
                Some(name) => {
                        let codec = decoder::find_by_name(name)
                            .expect("failed to find decoder");
                        unsafe {
                                codec::context::Context::wrap(avcodec_alloc_context3(codec.as_ptr()), None)
                        }
                }
                None => {
                        codec::context::Context::from_parameters(video_stream.parameters())?
                }
        };

        let decoder = codec_ctx.decoder().video()?;

        Ok(CodingContext {
                format_ctx,
                coder: decoder,
                video_stream_index
        })
}

fn open_decoder()