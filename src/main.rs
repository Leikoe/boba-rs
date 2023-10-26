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

use std::{env, ptr};
use std::collections::HashMap;
use std::time::Instant;

use ffmpeg::{codec, decoder, Dictionary, encoder, format, frame, log, media, Packet, picture, Rational};
use ffmpeg::sys::{av_guess_frame_rate, avcodec_alloc_context3};

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
                println!("Transcoder::new() - start");
                let global_header = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);

                let mut decoder_codec_context = ffmpeg::codec::context::Context::from_parameters(ist.parameters())?;
                let framerate = unsafe {
                        (*decoder_codec_context.as_mut_ptr()).framerate = av_guess_frame_rate(octx.as_mut_ptr(), ist.as_ptr().cast_mut(), ptr::null_mut());
                        Rational::from((*decoder_codec_context.as_mut_ptr()).framerate)
                };
                let decoder = decoder_codec_context
                    .decoder()
                    .video()?;

                let mut ost = octx.add_stream(None)?; // codec param is unused by ffmpeg
                let encoder_codec = encoder::find(codec::Id::H264).expect("couldn't find H265 codec");
                let encoder_codec_context = unsafe {
                        codec::context::Context::wrap(avcodec_alloc_context3(encoder_codec.as_ptr()), None)
                };
                let mut encoder = encoder_codec_context
                    .encoder()
                    .video()?;

                encoder.set_height(decoder.height());
                encoder.set_width(decoder.width());
                encoder.set_aspect_ratio(decoder.aspect_ratio());
                encoder.set_format(decoder.format());
                encoder.set_time_base(dbg!(framerate.invert()));
                dbg!(decoder.frame_rate(), decoder.time_base());
                // ost.set_time_base(framerate.invert());
                // ost.set_avg_frame_rate(framerate);
                // encoder.set_frame_rate(Some(framerate));

                if global_header {
                        encoder.set_flags(codec::Flags::GLOBAL_HEADER);
                }

                let encoder = encoder
                    .open_with(x264_opts)
                    .expect("error opening libx264 encoder with supplied settings");
                ost.set_parameters(&encoder);
                // {
                //         let c = ffmpeg::codec::context::Context::from_parameters(ost.parameters())?;
                //         println!("codec: {:?}", c.id());
                // }

                eprintln!("Transcoder::new() - end");
                Ok(Self {
                        ost_index,
                        decoder,
                        encoder: encoder.0,
                        logging_enabled: enable_logging,
                        frame_count: 0,
                        last_log_frame_count: 0,
                        starting_time: Instant::now(),
                        last_log_time: Instant::now(),
                })
        }

        fn send_packet_to_decoder(&mut self, packet: &Packet) {
                self.decoder.send_packet(packet).expect("Decoding failed");
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
                        let timestamp = frame.timestamp().unwrap_or(self.frame_count as i64);
                        self.log_progress(f64::from(
                                Rational(timestamp as i32, 1) * self.decoder.time_base()
                        ));
                        frame.set_pts(Some(timestamp));
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
                let mut p = Packet::empty();
                while self.encoder.receive_packet(&mut p).is_ok() {
                        p.set_stream(self.ost_index);
                        p.rescale_ts(self.decoder.time_base(), ost_time_base);
                        p.write_interleaved(octx).unwrap_or_else(|e| eprintln!("COULDN'T WRITE FRAME (err: {})", e));
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
                        "time elpased: \t{:8.2}\tframe count: {:8}\ttimestamp: {:8.20}",
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
        for (ist_index, mut ist) in ictx.streams().enumerate() {
                let ist_medium = ist.parameters().medium();
                if ist_medium != media::Type::Video
                // && ist_medium != media::Type::Audio
                // && ist_medium != media::Type::Subtitle
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
                                        ost_index as usize,
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
                ost_time_bases[ost_index] = octx.stream(ost_index).unwrap().time_base();
        }

        for (stream, mut packet) in ictx.packets() {
                let ist_index = stream.index();
                // println!("Demuxer gave frame of stream_index {ist_index}");
                let ost_index = stream_mapping[ist_index];
                if ost_index < 0 {
                        continue;
                }
                let ost_time_base = ost_time_bases[ost_index as usize];
                match transcoders.get_mut(&ist_index) {
                        Some(transcoder) => {
                                // println!("Going to reencode&filter the frame");
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
