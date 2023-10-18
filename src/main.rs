// #![feature(slice_take)]
// extern crate ffmpeg_the_third as ffmpeg;
//
// use std::env;
// use ffmpeg::media::Type;
// use ffmpeg::format::Pixel;
// use ffmpeg::{codec, encoder, format, frame, media, Packet, Rational, software};
//
//
// fn main() -> Result<(), ffmpeg::Error> {
//     ffmpeg::init().unwrap();
//
//     let input_file = env::args().nth(1).expect("missing input file");
//     let output_file = env::args().nth(2).expect("missing output file");
//
//     let mut ictx = format::input(&input_file).unwrap();
//     let mut octx = format::output(&output_file).unwrap();
//
//
//     let input = ictx
//         .streams()
//         .best(Type::Video)
//         .ok_or(ffmpeg::Error::StreamNotFound)?;
//     let video_stream_index = input.index();
//
//     octx.set_metadata(ictx.metadata().to_owned());
//     octx.write_header().unwrap();
//
//     let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
//     let mut decoder = context_decoder.decoder().video()?;
//
//     let mut scaler = ffmpeg::software::scaling::Context::get(
//         decoder.format(),
//         decoder.width(),
//         decoder.height(),
//         Pixel::RGB24,
//         decoder.width(),
//         decoder.height(),
//         ffmpeg::software::scaling::Flags::BILINEAR,
//     )?;
//
//     let mut encoder = codec::context::Context::from_parameters(input.parameters())?
//         .encoder()
//         .video()?;
//     encoder.set_format(decoder.format());
//     encoder.set_format(Pixel::RGB24);
//     encoder.set_width(decoder.width());
//     encoder.set_height(decoder.height());
//     encoder.set_aspect_ratio(decoder.aspect_ratio());
//     encoder.set_frame_rate(decoder.frame_rate());
//
//
//     let mut stream_mapping = vec![0; ictx.nb_streams() as _];
//     let mut ist_time_bases = vec![Rational(0, 1); ictx.nb_streams() as _];
//     let mut ost_index = 0;
//     for (ist_index, ist) in ictx.streams().enumerate() {
//         let ist_medium = ist.parameters().medium();
//         if ist_medium != media::Type::Audio
//             && ist_medium != media::Type::Video
//             && ist_medium != media::Type::Subtitle
//         {
//             stream_mapping[ist_index] = -1;
//             continue;
//         }
//         stream_mapping[ist_index] = ost_index;
//         ist_time_bases[ist_index] = ist.time_base();
//         ost_index += 1;
//         let mut ost = octx.add_stream(encoder::find(codec::Id::None)).unwrap();
//         ost.set_parameters(ist.parameters());
//         // We need to set codec_tag to 0 lest we run into incompatible codec tag
//         // issues when muxing into a different container format. Unfortunately
//         // there's no high level API to do this (yet).
//         unsafe {
//             (*ost.parameters().as_mut_ptr()).codec_tag = 0;
//         }
//     }
//
//
//     let weights = [0.5, 0.5];
//     let weights_sum: f64 = weights.iter().sum();
//
//     let normalized_weights: Vec<f64> = weights.iter().map(|x| x/weights_sum).collect();
//
//     let mut frames: Vec<frame::Video> = Vec::new();
//     let mut frame_index = 0;
//
//     let mut receive_and_process_decoded_frames =
//         |decoder: &mut ffmpeg::decoder::Video, encoder: &mut ffmpeg::encoder::video::Video| -> Result<(), ffmpeg::Error> {
//             let mut decoded = frame::Video::empty();
//             while decoder.receive_frame(&mut decoded).is_ok() {
//                 // // println!("new video frame");
//                 // let mut rgb_frame = frame::Video::empty();
//                 // scaler.run(&decoded, &mut rgb_frame)?;
//
//                 // frames.push(rgb_frame);
//                 // if frames.len() > normalized_weights.len() {
//                 //     let mut new_frame = frame::Video::new(Pixel::RGB24, encoder.width(), encoder.height());
//                 //     // let new_frame_data = frames
//                 //     //     .drain(0..normalized_weights.len())
//                 //     //     .map(|f| f.data(0).iter().map(|x| *x as f64).collect::<Vec<f64>>())
//                 //     //     .enumerate()
//                 //     //     .map(|(i, f)| (normalized_weights.get(i).expect("couldn't get normalized_weights corresponding to frame"), f))
//                 //     //     .map(
//                 //     //         |(w, f)| f.iter().map(|x| *x as f64 * w).collect::<Vec<f64>>()
//                 //     //     )
//                 //     //     .reduce(|a, b| a.iter().zip(b.iter()).map(|(x, y)| *x + *y).collect::<Vec<f64>>())
//                 //     //     .expect("lol");
//                 //     // new_frame.data_mut(0).iter_mut().zip(new_frame_data.iter()).for_each(|(o, i)| *o = *i as u8);
//                 //
//                 //     //dbg!(new_frame.data(0).len());
//                 //
//                 //     encoder.send_frame(&new_frame).expect("failed to send frame to encoder");
//                 // }
//                 encoder.send_frame(&decoded).expect("failed to send frame to encoder");
//
//
//                 frame_index += 1;
//             }
//             Ok(())
//         };
//
//     for (stream, packet) in ictx.packets() {
//         if stream.index() == video_stream_index {
//             decoder.send_packet(&packet)?;
//             receive_and_process_decoded_frames(&mut decoder, &mut encoder)?;
//
//             let mut new_packet = Packet::empty();
//             while encoder.receive_packet(&mut new_packet).is_ok() {
//                 new_packet.set_stream(packet.stream());
//                 new_packet.write_interleaved(&mut octx).unwrap();
//                 println!("wrote packet");
//             }
//         }
//     }
//     decoder.send_eof()?;
//     receive_and_process_decoded_frames(&mut decoder, &mut encoder)?;
//     octx.write_trailer().unwrap();
//
//     Ok(())
// }

// Given an input file, transcode all video streams into H.264 (using libx264)
// while copying audio and subtitle streams.
//
// Invocation:
//
//   transcode-x264 <input> <output> [<x264_opts>]
//
// <x264_opts> is a comma-delimited list of key=val. default is "preset=medium".
// See https://ffmpeg.org/ffmpeg-codecs.html#libx264_002c-libx264rgb and
// https://trac.ffmpeg.org/wiki/Encode/H.264 for available and commonly used
// options.
//
// Examples:
//
//   transcode-x264 input.flv output.mp4
//   transcode-x264 input.mkv output.mkv 'preset=veryslow,crf=18'

extern crate ffmpeg_the_third as ffmpeg;

use std::collections::HashMap;
use std::env;
use std::time::Instant;

use ffmpeg::{
    codec, decoder, encoder, format, frame, log, media, picture, Dictionary, Packet, Rational,
};

const DEFAULT_X264_OPTS: &str = "preset=medium";

struct Transcoder {
    ost_index: usize,
    decoder: decoder::Video,
    encoder: encoder::video::Video,
    logging_enabled: bool,
    frame_count: usize,
    last_log_frame_count: usize,
    starting_time: Instant,
    last_log_time: Instant,
}

impl Transcoder {
    fn new(
        ist: &format::stream::Stream,
        octx: &mut format::context::Output,
        ost_index: usize,
        x264_opts: Dictionary,
        enable_logging: bool,
    ) -> Result<Self, ffmpeg::Error> {
        let global_header = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);
        let decoder = ffmpeg::codec::context::Context::from_parameters(ist.parameters())?
            .decoder()
            .video()?;
        let mut ost = octx.add_stream(encoder::find(codec::Id::H264))?;
        let mut encoder = codec::context::Context::from_parameters(ost.parameters())?
            .encoder()
            .video()?;
        encoder.set_height(decoder.height());
        encoder.set_width(decoder.width());
        encoder.set_aspect_ratio(decoder.aspect_ratio());
        encoder.set_format(decoder.format());
        encoder.set_frame_rate(decoder.frame_rate());
        encoder.set_time_base(Rational(1, 30));
        if global_header {
            encoder.set_flags(codec::Flags::GLOBAL_HEADER);
        }

        encoder
            .open_with(x264_opts)
            .expect("error opening libx264 encoder with supplied settings");
        encoder = codec::context::Context::from_parameters(ost.parameters())?
            .encoder()
            .video()?;
        ost.set_parameters(&encoder);
        Ok(Self {
            ost_index,
            decoder,
            encoder: codec::context::Context::from_parameters(ost.parameters())?
                .encoder()
                .video()?,
            logging_enabled: enable_logging,
            frame_count: 0,
            last_log_frame_count: 0,
            starting_time: Instant::now(),
            last_log_time: Instant::now(),
        })
    }

    fn send_packet_to_decoder(&mut self, packet: &Packet) {
        self.decoder.send_packet(packet).unwrap();
    }

    fn send_eof_to_decoder(&mut self) {
        self.decoder.send_eof().unwrap();
    }

    fn receive_and_process_decoded_frames(
        &mut self,
        octx: &mut format::context::Output,
        ost_time_base: Rational,
    ) {
        let mut frame = frame::Video::empty();
        while self.decoder.receive_frame(&mut frame).is_ok() {
            self.frame_count += 1;
            let timestamp = frame.timestamp();
            self.log_progress(f64::from(
                Rational(timestamp.unwrap_or(0) as i32, 1) * self.decoder.time_base(),
            ));
            frame.set_pts(timestamp);
            frame.set_kind(picture::Type::None);
            self.send_frame_to_encoder(&frame);
            self.receive_and_process_encoded_packets(octx, ost_time_base);
        }
    }

    fn send_frame_to_encoder(&mut self, frame: &frame::Video) {
        self.encoder.send_frame(frame).unwrap();
    }

    fn send_eof_to_encoder(&mut self) {
        self.encoder.send_eof().unwrap();
    }

    fn receive_and_process_encoded_packets(
        &mut self,
        octx: &mut format::context::Output,
        ost_time_base: Rational,
    ) {
        let mut encoded = Packet::empty();
        while self.encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(self.ost_index);
            encoded.rescale_ts(self.decoder.time_base(), ost_time_base);
            encoded.write_interleaved(octx).unwrap();
        }
    }

    fn log_progress(&mut self, timestamp: f64) {
        if !self.logging_enabled
            || (self.frame_count - self.last_log_frame_count < 100
            && self.last_log_time.elapsed().as_secs_f64() < 1.0)
        {
            return;
        }
        eprintln!(
            "time elpased: \t{:8.2}\tframe count: {:8}\ttimestamp: {:8.2}",
            self.starting_time.elapsed().as_secs_f64(),
            self.frame_count,
            timestamp
        );
        self.last_log_frame_count = self.frame_count;
        self.last_log_time = Instant::now();
    }
}

fn parse_opts<'a>(s: String) -> Option<Dictionary<'a>> {
    let mut dict = Dictionary::new();
    for keyval in s.split_terminator(',') {
        let tokens: Vec<&str> = keyval.split('=').collect();
        match tokens[..] {
            [key, val] => dict.set(key, val),
            _ => return None,
        }
    }
    Some(dict)
}

fn main() {
    let input_file = env::args().nth(1).expect("missing input file");
    let output_file = env::args().nth(2).expect("missing output file");
    let x264_opts = parse_opts(
        env::args()
            .nth(3)
            .unwrap_or_else(|| DEFAULT_X264_OPTS.to_string()),
    )
        .expect("invalid x264 options string");

    eprintln!("x264 options: {x264_opts:?}");

    ffmpeg::init().unwrap();
    log::set_level(log::Level::Info);

    let mut ictx = format::input(&input_file).unwrap();
    let mut octx = format::output(&output_file).unwrap();

    format::context::input::dump(&ictx, 0, Some(&input_file));

    let best_video_stream_index = ictx
        .streams()
        .best(media::Type::Video)
        .map(|stream| stream.index());
    let mut stream_mapping: Vec<isize> = vec![0; ictx.nb_streams() as _];
    let mut ist_time_bases = vec![Rational(0, 0); ictx.nb_streams() as _];
    let mut ost_time_bases = vec![Rational(0, 0); ictx.nb_streams() as _];
    let mut transcoders = HashMap::new();
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
        if ist_medium == media::Type::Video {
            // Initialize transcoder for video stream.
            transcoders.insert(
                ist_index,
                Transcoder::new(
                    &ist,
                    &mut octx,
                    ost_index as _,
                    x264_opts.to_owned(),
                    Some(ist_index) == best_video_stream_index,
                )
                    .unwrap(),
            );
        } else {
            // Set up for stream copy for non-video stream.
            let mut ost = octx.add_stream(encoder::find(codec::Id::None)).unwrap();
            ost.set_parameters(ist.parameters());
            // We need to set codec_tag to 0 lest we run into incompatible codec tag
            // issues when muxing into a different container format. Unfortunately
            // there's no high level API to do this (yet).
            unsafe {
                (*ost.parameters().as_mut_ptr()).codec_tag = 0;
            }
        }
        ost_index += 1;
    }

    octx.set_metadata(ictx.metadata().to_owned());
    format::context::output::dump(&octx, 0, Some(&output_file));
    octx.write_header().unwrap();

    for (ost_index, _) in octx.streams().enumerate() {
        ost_time_bases[ost_index] = octx.stream(ost_index as _).unwrap().time_base();
    }

    for (stream, mut packet) in ictx.packets() {
        let ist_index = stream.index();
        let ost_index = stream_mapping[ist_index];
        if ost_index < 0 {
            continue;
        }
        let ost_time_base = ost_time_bases[ost_index as usize];
        match transcoders.get_mut(&ist_index) {
            Some(transcoder) => {
                packet.rescale_ts(stream.time_base(), transcoder.decoder.time_base());
                transcoder.send_packet_to_decoder(&packet);
                transcoder.receive_and_process_decoded_frames(&mut octx, ost_time_base);
            }
            None => {
                // Do stream copy on non-video streams.
                packet.rescale_ts(ist_time_bases[ist_index], ost_time_base);
                packet.set_position(-1);
                packet.set_stream(ost_index as _);
                packet.write_interleaved(&mut octx).unwrap();
            }
        }
    }

    // Flush encoders and decoders.
    for (ost_index, transcoder) in transcoders.iter_mut() {
        let ost_time_base = ost_time_bases[*ost_index];
        transcoder.send_eof_to_decoder();
        transcoder.receive_and_process_decoded_frames(&mut octx, ost_time_base);
        transcoder.send_eof_to_encoder();
        transcoder.receive_and_process_encoded_packets(&mut octx, ost_time_base);
    }

    octx.write_trailer().unwrap();
}