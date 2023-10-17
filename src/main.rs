#![feature(slice_take)]
extern crate ffmpeg_the_third as ffmpeg;

use ffmpeg::format::{input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context, flag::Flags};
use ffmpeg::util::frame::video::Video;
use std::env;
use ffmpeg::{codec, encoder, format, media, Packet, Rational};


fn main() -> Result<(), ffmpeg::Error> {
    ffmpeg::init().unwrap();

    let input_file = env::args().nth(1).expect("missing input file");
    let output_file = env::args().nth(2).expect("missing output file");

    let mut ictx = format::input(&input_file).unwrap();
    let mut octx = format::output(&output_file).unwrap();

    let mut stream_mapping = vec![0; ictx.nb_streams() as _];
    let mut ist_time_bases = vec![Rational(0, 1); ictx.nb_streams() as _];
    let mut ost_index = 0;
    for (ist_index, ist) in ictx.streams().enumerate() {
        let ist_medium = ist.parameters().medium();
        if ist_medium != media::Type::Audio
            && ist_medium != media::Type::Video
            && ist_medium != media::Type::Subtitle
        {
            stream_mapping[ist_index] = -1;
            continue;
        }
        stream_mapping[ist_index] = ost_index;
        ist_time_bases[ist_index] = ist.time_base();
        ost_index += 1;
        let mut ost = octx.add_stream(encoder::find(codec::Id::None)).unwrap();
        ost.set_parameters(ist.parameters());
        // We need to set codec_tag to 0 lest we run into incompatible codec tag
        // issues when muxing into a different container format. Unfortunately
        // there's no high level API to do this (yet).
        unsafe {
            (*ost.parameters().as_mut_ptr()).codec_tag = 0;
        }
    }


    let input = ictx
        .streams()
        .best(Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;
    let video_stream_index = input.index();

    octx.set_metadata(ictx.metadata().to_owned());
    octx.write_header().unwrap();

    let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
    let mut decoder = context_decoder.decoder().video()?;
    let context_encoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
    let mut encoder = context_encoder.encoder().video()?;

    let mut scaler = Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )?;


    let weights = [0.5, 0.5];
    let weights_sum: f64 = weights.iter().sum();

    let normalized_weights: Vec<f64> = weights.iter().map(|x| x/weights_sum).collect();

    let mut frames = Vec::new();
    let mut frame_index = 0;

    let mut receive_and_process_decoded_frames =
        |decoder: &mut ffmpeg::decoder::Video, encoder: &mut ffmpeg::encoder::video::Video| -> Result<(), ffmpeg::Error> {
            let mut decoded = Video::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
                // println!("new video frame");
                let mut rgb_frame = Video::empty();
                scaler.run(&decoded, &mut rgb_frame)?;

                frames.push(rgb_frame);
                if frames.len() > normalized_weights.len() {
                    let mut new_frame = Video::new(Pixel::RGB24, decoder.width(), decoder.height());
                    let new_frame_data = frames
                        .drain(0..normalized_weights.len())
                        .map(|f| f.data(0).iter().map(|x| *x as f64).collect::<Vec<f64>>())
                        .enumerate()
                        .map(|(i, f)| (normalized_weights.get(i).expect("couldn't get normalized_weights corresponding to frame"), f))
                        .map(
                            |(w, f)| f.iter().map(|x| *x as f64 * w).collect::<Vec<f64>>()
                        )
                        .reduce(|a, b| a.iter().zip(b.iter()).map(|(x, y)| *x + *y).collect::<Vec<f64>>())
                        .expect("lol");
                    new_frame.data_mut(0).iter_mut().zip(new_frame_data.iter()).for_each(|(o, i)| *o = *i as u8);

                    dbg!(new_frame.data(0).len());

                    encoder.send_frame(&new_frame).expect("failed to send frame to encoder");
                }

                frame_index += 1;
            }
            Ok(())
        };

    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet)?;
            receive_and_process_decoded_frames(&mut decoder, &mut encoder)?;

            let mut new_packet = packet.clone();
            while encoder.receive_packet(&mut new_packet).is_ok() {
                new_packet.set_stream(video_stream_index);
                new_packet.write_interleaved(&mut octx).unwrap();
                println!("wrote packet");
            }
        }
    }
    decoder.send_eof()?;
    receive_and_process_decoded_frames(&mut decoder, &mut encoder)?;
    octx.write_trailer().unwrap();

    Ok(())
}
