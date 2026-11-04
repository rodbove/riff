use std::path::{Path, PathBuf};
use std::sync::Arc;

use riff_dsp::wav;
use serde::{Deserialize, Serialize};

pub const PEAK_BLOCK: usize = 256;
const UNDO: usize = 50;

#[derive(Clone)]
pub struct Source {
    pub id: u64,
    pub dry: Arc<Vec<f32>>,
    pub wet: Arc<Vec<[f32; 2]>>,
    pub peaks: Arc<Vec<f32>>,
}

impl Source {
    pub fn new(id: u64, dry: Vec<f32>, wet: Vec<[f32; 2]>) -> Self {
        let peaks = peaks(&wet);
        Self {
            id,
            dry: Arc::new(dry),
            wet: Arc::new(wet),
            peaks: Arc::new(peaks),
        }
    }

    pub fn len(&self) -> usize {
        self.wet.len().max(self.dry.len())
    }

    pub fn rewet(&self, wet: Vec<[f32; 2]>) -> Self {
        Self::new(self.id, self.dry.to_vec(), wet)
    }
}

fn peaks(wet: &[[f32; 2]]) -> Vec<f32> {
    wet.chunks(PEAK_BLOCK)
        .map(|c| {
            c.iter()
                .fold(0.0f32, |m, f| m.max(f[0].abs()).max(f[1].abs()))
        })
        .collect()
}

#[derive(Clone)]
pub struct Clip {
    pub source: Source,
    pub start: usize,
    pub offset: usize,
    pub len: usize,
}

impl Clip {
    pub fn whole(source: Source, start: usize) -> Self {
        let len = source.len();
        Self {
            source,
            start,
            offset: 0,
            len,
        }
    }

    pub fn end(&self) -> usize {
        self.start + self.len
    }

    fn wet(&self, i: usize) -> [f32; 2] {
        self.source
            .wet
            .get(self.offset + i)
            .copied()
            .unwrap_or([0.0; 2])
    }

    fn dry(&self, i: usize) -> f32 {
        self.source.dry.get(self.offset + i).copied().unwrap_or(0.0)
    }
}

#[derive(Clone)]
pub struct Track {
    pub name: String,
    pub clips: Vec<Clip>,
    pub mute: bool,
    pub solo: bool,
    pub volume: f32,
}

impl Track {
    pub fn new(name: String) -> Self {
        Self {
            name,
            clips: Vec::new(),
            mute: false,
            solo: false,
            volume: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct At {
    pub track: usize,
    pub clip: usize,
}

pub struct Session {
    pub rate: u32,
    pub tracks: Vec<Track>,
    pub next_id: u64,
    history: Vec<Vec<Track>>,
}

fn gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

impl Session {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            tracks: vec![Track::new("TRACK 1".into())],
            next_id: 1,
            history: Vec::new(),
        }
    }

    pub fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn remember(&mut self) {
        self.history.push(self.tracks.clone());
        if self.history.len() > UNDO {
            self.history.remove(0);
        }
    }

    pub fn undo(&mut self) -> bool {
        match self.history.pop() {
            Some(t) => {
                self.tracks = t;
                true
            }
            None => false,
        }
    }

    pub fn length(&self) -> usize {
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(Clip::end))
            .max()
            .unwrap_or(0)
    }

    pub fn add_track(&mut self) -> usize {
        self.remember();
        let n = self.tracks.len() + 1;
        self.tracks.push(Track::new(format!("TRACK {n}")));
        self.tracks.len() - 1
    }

    pub fn remove_track(&mut self, track: usize) {
        if track < self.tracks.len() {
            self.remember();
            self.tracks.remove(track);
        }
    }

    pub fn audible(&self, track: usize) -> bool {
        let soloing = self.tracks.iter().any(|t| t.solo);
        let t = &self.tracks[track];
        !t.mute && (!soloing || t.solo)
    }

    pub fn mix(&self, from: usize, skip: Option<usize>) -> Vec<[f32; 2]> {
        let len = self.length().saturating_sub(from);
        let mut out = vec![[0.0f32; 2]; len];
        for (ti, t) in self.tracks.iter().enumerate() {
            if Some(ti) == skip || !self.audible(ti) {
                continue;
            }
            let g = gain(t.volume);
            for c in &t.clips {
                let begin = c.start.max(from);
                for pos in begin..c.end() {
                    let f = c.wet(pos - c.start);
                    let o = &mut out[pos - from];
                    o[0] += f[0] * g;
                    o[1] += f[1] * g;
                }
            }
        }
        out
    }

    pub fn di(&self, track: usize) -> Vec<f32> {
        let t = &self.tracks[track];
        let len = t.clips.iter().map(Clip::end).max().unwrap_or(0);
        let mut out = vec![0.0f32; len];
        for c in &t.clips {
            for i in 0..c.len {
                out[c.start + i] += c.dry(i);
            }
        }
        out
    }

    pub fn place(&mut self, track: usize, clip: Clip) -> At {
        self.remember();
        let t = &mut self.tracks[track];
        t.clips.push(clip);
        At {
            track,
            clip: t.clips.len() - 1,
        }
    }

    pub fn clip_at(&self, track: usize, pos: usize) -> Option<At> {
        let t = self.tracks.get(track)?;
        t.clips
            .iter()
            .rposition(|c| c.start <= pos && pos < c.end())
            .map(|clip| At { track, clip })
    }

    pub fn clip(&self, at: At) -> Option<&Clip> {
        self.tracks.get(at.track)?.clips.get(at.clip)
    }

    pub fn split(&mut self, at: At, pos: usize) -> bool {
        let Some(c) = self.clip(at).cloned() else {
            return false;
        };
        if pos <= c.start || pos >= c.end() {
            return false;
        }
        self.remember();
        let cut = pos - c.start;
        let right = Clip {
            source: c.source.clone(),
            start: pos,
            offset: c.offset + cut,
            len: c.len - cut,
        };
        let t = &mut self.tracks[at.track];
        t.clips[at.clip].len = cut;
        t.clips.insert(at.clip + 1, right);
        true
    }

    pub fn delete(&mut self, at: At) -> bool {
        if self.clip(at).is_none() {
            return false;
        }
        self.remember();
        self.tracks[at.track].clips.remove(at.clip);
        true
    }

    pub fn shift(&mut self, at: At, start: usize) {
        if let Some(c) = self
            .tracks
            .get_mut(at.track)
            .and_then(|t| t.clips.get_mut(at.clip))
        {
            c.start = start;
        }
    }

    pub fn trim_front(&mut self, at: At, pos: usize) {
        if let Some(c) = self
            .tracks
            .get_mut(at.track)
            .and_then(|t| t.clips.get_mut(at.clip))
        {
            let earliest = c.start - c.offset;
            let pos = pos.clamp(earliest, c.end().saturating_sub(1));
            let end = c.end();
            c.offset = pos - earliest;
            c.start = pos;
            c.len = end - pos;
        }
    }

    pub fn trim_back(&mut self, at: At, pos: usize) {
        if let Some(c) = self
            .tracks
            .get_mut(at.track)
            .and_then(|t| t.clips.get_mut(at.clip))
        {
            let latest = c.start + c.source.len() - c.offset;
            c.len = pos.clamp(c.start + 1, latest) - c.start;
        }
    }

    pub fn sources(&self, track: usize) -> Vec<Source> {
        let mut out: Vec<Source> = Vec::new();
        for c in &self.tracks[track].clips {
            if !out.iter().any(|s| s.id == c.source.id) {
                out.push(c.source.clone());
            }
        }
        out
    }

    pub fn replace(&mut self, source: Source) {
        for t in &mut self.tracks {
            for c in &mut t.clips {
                if c.source.id == source.id {
                    c.source = source.clone();
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SavedClip {
    source: u64,
    start: usize,
    offset: usize,
    len: usize,
}

#[derive(Serialize, Deserialize)]
struct SavedTrack {
    name: String,
    mute: bool,
    solo: bool,
    volume: f32,
    clips: Vec<SavedClip>,
}

#[derive(Serialize, Deserialize)]
struct Saved {
    rate: u32,
    next_id: u64,
    tracks: Vec<SavedTrack>,
}

fn dry_file(dir: &Path, id: u64) -> PathBuf {
    dir.join("clips").join(format!("{id} DI.wav"))
}

fn wet_file(dir: &Path, id: u64) -> PathBuf {
    dir.join("clips").join(format!("{id} riff.wav"))
}

pub fn write_source(dir: &Path, s: &Source, rate: u32) -> Result<(), String> {
    let wet: Vec<f32> = s.wet.iter().flatten().copied().collect();
    wav::write(&dry_file(dir, s.id), rate, 1, &s.dry)?;
    wav::write(&wet_file(dir, s.id), rate, 2, &wet)
}

pub fn save(dir: &Path, session: &Session) -> Result<(), String> {
    let saved = Saved {
        rate: session.rate,
        next_id: session.next_id,
        tracks: session
            .tracks
            .iter()
            .map(|t| SavedTrack {
                name: t.name.clone(),
                mute: t.mute,
                solo: t.solo,
                volume: t.volume,
                clips: t
                    .clips
                    .iter()
                    .map(|c| SavedClip {
                        source: c.source.id,
                        start: c.start,
                        offset: c.offset,
                        len: c.len,
                    })
                    .collect(),
            })
            .collect(),
    };
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("session.json"), text).map_err(|e| e.to_string())
}

pub fn load(dir: &Path) -> Result<Session, String> {
    let text = std::fs::read_to_string(dir.join("session.json")).map_err(|e| e.to_string())?;
    let saved: Saved = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut sources: Vec<Source> = Vec::new();
    let mut tracks = Vec::new();
    for st in saved.tracks {
        let mut t = Track::new(st.name);
        t.mute = st.mute;
        t.solo = st.solo;
        t.volume = st.volume;
        for sc in st.clips {
            let source = match sources.iter().find(|s| s.id == sc.source) {
                Some(s) => s.clone(),
                None => {
                    let dry = wav::read(&dry_file(dir, sc.source))?.samples;
                    let wet = wav::read(&wet_file(dir, sc.source))?
                        .samples
                        .chunks(2)
                        .map(|f| [f[0], *f.get(1).unwrap_or(&f[0])])
                        .collect();
                    let s = Source::new(sc.source, dry, wet);
                    sources.push(s.clone());
                    s
                }
            };
            t.clips.push(Clip {
                source,
                start: sc.start,
                offset: sc.offset,
                len: sc.len,
            });
        }
        tracks.push(t);
    }
    if tracks.is_empty() {
        tracks.push(Track::new("TRACK 1".into()));
    }
    Ok(Session {
        rate: saved.rate,
        tracks,
        next_id: saved.next_id,
        history: Vec::new(),
    })
}

pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let len = (input.len() as f64 / ratio) as usize;
    (0..len)
        .map(|i| {
            let x = i as f64 * ratio;
            let j = x as usize;
            let t = (x - j as f64) as f32;
            let a = input[j];
            let b = *input.get(j + 1).unwrap_or(&a);
            a + (b - a) * t
        })
        .collect()
}

pub fn import(path: &Path, rate: u32) -> Result<(Vec<f32>, Vec<[f32; 2]>), String> {
    let audio = wav::read(path)?;
    let ch = audio.channels.max(1);
    let left: Vec<f32> = audio.samples.chunks(ch).map(|f| f[0]).collect();
    let right: Vec<f32> = audio
        .samples
        .chunks(ch)
        .map(|f| *f.get(1).unwrap_or(&f[0]))
        .collect();
    let left = resample(&left, audio.rate, rate);
    let right = resample(&right, audio.rate, rate);
    let dry: Vec<f32> = left
        .iter()
        .zip(&right)
        .map(|(l, r)| (l + r) * 0.5)
        .collect();
    let wet = left.into_iter().zip(right).map(|(l, r)| [l, r]).collect();
    Ok((dry, wet))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: u64, len: usize, value: f32) -> Source {
        Source::new(id, vec![value; len], vec![[value, value]; len])
    }

    fn session() -> Session {
        let mut s = Session::new(48000);
        s.place(0, Clip::whole(source(1, 100, 0.5), 10));
        s
    }

    const FIRST: At = At { track: 0, clip: 0 };

    mod when_mixing {
        use super::*;

        #[test]
        fn places_clips_at_their_start() {
            let m = session().mix(0, None);
            assert_eq!(m.len(), 110);
            assert_eq!(m[9], [0.0, 0.0]);
            assert_eq!(m[10], [0.5, 0.5]);
        }

        #[test]
        fn starts_at_the_playhead() {
            let m = session().mix(60, None);
            assert_eq!(m.len(), 50);
            assert_eq!(m[0], [0.5, 0.5]);
        }

        mod with_a_muted_track {
            use super::*;

            #[test]
            fn leaves_it_out() {
                let mut s = session();
                s.tracks[0].mute = true;
                assert!(s.mix(0, None).iter().all(|f| f[0] == 0.0));
            }
        }

        mod with_a_soloed_track {
            use super::*;

            #[test]
            fn plays_only_that_track() {
                let mut s = session();
                let t = s.add_track();
                s.place(t, Clip::whole(source(2, 50, 0.25), 0));
                s.tracks[t].solo = true;
                assert_eq!(s.mix(0, None)[20], [0.25, 0.25]);
            }
        }

        mod while_recording_a_track {
            use super::*;

            #[test]
            fn skips_that_track() {
                assert!(session().mix(0, Some(0)).iter().all(|f| f[0] == 0.0));
            }
        }
    }

    mod when_splitting {
        use super::*;

        #[test]
        fn makes_two_clips_that_play_the_same() {
            let mut s = session();
            let before = s.mix(0, None);
            assert!(s.split(FIRST, 40));
            assert_eq!(s.tracks[0].clips.len(), 2);
            assert_eq!(s.tracks[0].clips[1].offset, 30);
            assert_eq!(s.mix(0, None), before);
        }

        mod outside_the_clip {
            use super::*;

            #[test]
            fn does_nothing() {
                let mut s = session();
                assert!(!s.split(FIRST, 10));
                assert!(!s.split(FIRST, 200));
            }
        }
    }

    mod when_trimming {
        use super::*;

        #[test]
        fn keeps_audio_in_place() {
            let mut s = session();
            s.trim_front(FIRST, 30);
            let c = &s.tracks[0].clips[0];
            assert_eq!((c.start, c.offset, c.len), (30, 20, 80));
            s.trim_back(FIRST, 50);
            assert_eq!(s.tracks[0].clips[0].len, 20);
        }

        #[test]
        fn stops_at_the_recorded_audio() {
            let mut s = session();
            s.trim_front(FIRST, 0);
            s.trim_back(FIRST, 500);
            let c = &s.tracks[0].clips[0];
            assert_eq!((c.start, c.offset, c.len), (10, 0, 100));
        }
    }

    mod when_undoing {
        use super::*;

        #[test]
        fn restores_the_last_edit() {
            let mut s = session();
            s.delete(FIRST);
            assert!(s.tracks[0].clips.is_empty());
            assert!(s.undo());
            assert_eq!(s.tracks[0].clips.len(), 1);
        }
    }

    mod when_exporting_the_di {
        use super::*;

        #[test]
        fn renders_the_track_dry() {
            let mut s = session();
            s.split(FIRST, 40);
            s.delete(At { track: 0, clip: 1 });
            let di = s.di(0);
            assert_eq!(di.len(), 40);
            assert_eq!(di[39], 0.5);
            assert_eq!(di[0], 0.0);
        }
    }

    mod when_saving_and_loading {
        use super::*;

        #[test]
        fn keeps_tracks_and_clips() {
            let dir = std::env::temp_dir().join(format!("riff-studio-{}", std::process::id()));
            let mut s = session();
            s.split(FIRST, 40);
            s.tracks[0].volume = -3.0;
            for src in s.sources(0) {
                write_source(&dir, &src, s.rate).unwrap();
            }
            save(&dir, &s).unwrap();
            let back = load(&dir).unwrap();
            std::fs::remove_dir_all(&dir).ok();
            assert_eq!(back.tracks[0].clips.len(), 2);
            assert_eq!(back.tracks[0].volume, -3.0);
            assert_eq!(back.mix(0, None), s.mix(0, None));
        }
    }

    mod when_resampling {
        use super::*;

        #[test]
        fn changes_the_length_by_the_rate_ratio() {
            let x: Vec<f32> = (0..441).map(|i| i as f32).collect();
            let y = resample(&x, 44100, 48000);
            assert_eq!(y.len(), 480);
            assert!((y[480 - 1] - 440.0).abs() < 1.0);
        }
    }
}
