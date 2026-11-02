use crate::amp::AmpModel;
use crate::cab::{CabModel, CabState};
use crate::fx::Kind;
use crate::rig::{AmpState, GateState, Rig, Slot};

pub const BANDS: [&str; 7] = [
    "METALLICA",
    "DREAM THEATER",
    "DEATH",
    "AVENGED SEVENFOLD",
    "ARCH ENEMY",
    "METALCORE",
    "EXTRAS",
];
pub const USER: &str = "USER";

struct Part {
    name: &'static str,
    note: &'static str,
    gate: f32,
    pre: Vec<Slot>,
    amp: AmpState,
    cab: CabState,
    post: Vec<Slot>,
}

fn part(
    name: &'static str,
    note: &'static str,
    gate: f32,
    pre: Vec<Slot>,
    amp: AmpState,
    cab: CabState,
    post: Vec<Slot>,
) -> Part {
    Part {
        name,
        note,
        gate,
        pre,
        amp,
        cab,
        post,
    }
}

fn album(band: &str, album: &str, parts: Vec<Part>) -> Vec<Rig> {
    parts
        .into_iter()
        .map(|p| Rig {
            name: p.name.into(),
            category: band.into(),
            album: album.into(),
            note: p.note.into(),
            input: 0.0,
            output: -6.0,
            gate: GateState {
                on: p.gate < 0.0,
                threshold: if p.gate < 0.0 { p.gate } else { -60.0 },
                release: if p.gate > -51.0 && p.gate < 0.0 {
                    40.0
                } else {
                    60.0
                },
            },
            pre: p.pre,
            amp: p.amp,
            cab: p.cab,
            post: p.post,
        })
        .collect()
}

fn cab(model: CabModel, blend: f32, low_cut: f32, high_cut: f32) -> CabState {
    CabState {
        values: vec![blend, low_cut, high_cut, 0.0],
        ..CabState::with(model)
    }
}

fn v30() -> CabState {
    cab(CabModel::V30, 0.3, 80.0, 9000.0)
}

fn heavy_cab() -> CabState {
    cab(CabModel::V30, 0.35, 80.0, 9500.0)
}

fn open_cab() -> CabState {
    cab(CabModel::OpenBack, 0.35, 60.0, 11000.0)
}

fn no_cab() -> CabState {
    CabState {
        on: false,
        ..open_cab()
    }
}

fn od(drive: f32, tone: f32, level: f32) -> Slot {
    Slot::with(
        Kind::Overdrive,
        &[("DRIVE", drive), ("TONE", tone), ("LEVEL", level)],
    )
}

fn v_curve(depth: f32) -> Slot {
    Slot::with(
        Kind::PostEq,
        &[
            ("62", 2.0 * depth),
            ("125", 3.0 * depth),
            ("500", -3.0 * depth),
            ("1K", -3.5 * depth),
            ("2K", -depth),
            ("4K", 1.5 * depth),
            ("8K", 2.0 * depth),
        ],
    )
}

fn doubler(width: f32) -> Slot {
    Slot::with(Kind::Doubler, &[("WIDTH", width), ("DELAY", 16.0)])
}

fn delay(ms: f32, feedback: f32, mix: f32, mode: f32) -> Slot {
    Slot::with(
        Kind::Delay,
        &[
            ("TIME", ms),
            ("FEEDBACK", feedback),
            ("MIX", mix),
            ("TONE", 4500.0),
            ("MODE", mode),
        ],
    )
}

fn verb(kind: f32, decay: f32, mix: f32) -> Slot {
    Slot::with(
        Kind::Reverb,
        &[("TYPE", kind), ("DECAY", decay), ("MIX", mix)],
    )
}

fn chorus(rate: f32, depth: f32, mix: f32) -> Slot {
    Slot::with(
        Kind::Chorus,
        &[("RATE", rate), ("DEPTH", depth), ("MIX", mix)],
    )
}

fn comp() -> Slot {
    Slot::with(
        Kind::Compressor,
        &[
            ("THRESH", -28.0),
            ("RATIO", 3.0),
            ("ATTACK", 10.0),
            ("RELEASE", 200.0),
            ("MAKEUP", 6.0),
        ],
    )
}

fn wah(position: f32) -> Slot {
    Slot::with(Kind::Wah, &[("PEDAL", position)])
}

fn harmony(interval: f32, mix: f32) -> Slot {
    Slot::with(
        Kind::Harmony,
        &[("INTERVAL", interval), ("MIX", mix), ("WIDTH", 0.8)],
    )
}

fn tune(semis: f32) -> Slot {
    Slot {
        on: false,
        ..Slot::with(Kind::Tune, &[("SHIFT", semis)])
    }
}

fn acoustic() -> Slot {
    Slot::with(Kind::Acoustic, &[("BODY", 6.0), ("TOP", 6.0)])
}

fn amp(model: AmpModel, v: [f32; 8]) -> AmpState {
    AmpState::with(model, v)
}

const DIGITAL: f32 = 0.0;
const ANALOG: f32 = 1.0;
const PING_PONG: f32 = 2.0;
const ROOM: f32 = 0.0;
const PLATE: f32 = 1.0;
const HALL: f32 = 2.0;
const MAJOR_THIRD: f32 = 5.0;
const MINOR_THIRD: f32 = 4.0;

const TRIMS: &[(&str, f32)] = &[
    ("Fight Fire with Fire Intro", 7e+00),
    ("Fight Fire with Fire", -8e+00),
    ("For Whom the Bell Tolls", -9e+00),
    ("Fade to Black Clean", 8e+00),
    ("Fade to Black Solo", -8e+00),
    ("The Call of Ktulu", -4e+00),
    ("Puppets Rhythm", -8e+00),
    ("Puppets Interlude Clean", 8e+00),
    ("That Was Just Your Life", -4e+00),
    ("The End of the Line", -4e+00),
    ("The Day That Never Comes Clean", 7e+00),
    ("The Day That Never Comes Heavy", -4e+00),
    ("All Nightmare Long", -4e+00),
    ("Endless Sacrifice Clean", 7e+00),
    ("Endless Sacrifice Heavy", -4e+00),
    ("Stream of Consciousness", -4e+00),
    ("Petrucci Lead ToT", -4e+00),
    ("Petrucci Rhythm", -4e+00),
    ("Petrucci Lead", -4e+00),
    ("Petrucci Clean", 8e+00),
    ("Bat Country Rhythm", -8e+00),
    ("Bat Country Solo", -8e+00),
    ("Beast and the Harlot", -8e+00),
    ("Seize the Day Clean", 7e+00),
    ("Seize the Day Solo", -8e+00),
    ("Critical Acclaim", -4e+00),
    ("Almost Easy", -4e+00),
    ("Scream", -4e+00),
    ("Afterlife Rhythm", -8e+00),
    ("Afterlife Solo", -8e+00),
    ("Gunslinger Acoustic", 6e+00),
    ("Dear God Clean", 6e+00),
    ("Unbound", -4e+00),
    ("A Little Piece of Heaven", -8e+00),
    ("A7X Rhythm", -4e+00),
    ("Synyster Lead", -8e+00),
    ("Vultures Clean", 7e+00),
    ("Amott Twin Lead", -8e+00),
    ("Amott Lead", -8e+00),
    ("Swedish Death", -8e+00),
    ("Clean Room", 4e+00),
    ("One Clean", 7e+00),
    ("One Solo", -4e+00),
    ("Harvester of Sorrow", -4e+00),
    ("To Live Is to Die Clean", 8e+00),
    ("We Will Rise Solo", -9e+00),
    ("Clean Arpeggios", 7e+00),
    ("Melodic Lead", -8e+00),
    ("Ambient Clean", 6e+00),
];

pub fn factory() -> Vec<Rig> {
    let mut bank = bank();
    for r in &mut bank {
        if let Some((_, out)) = TRIMS.iter().find(|(n, _)| *n == r.name) {
            r.output = *out;
        }
    }
    bank
}

pub fn albums(bank: &[Rig], band: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in bank.iter().filter(|r| r.category == band) {
        if !out.contains(&r.album) {
            out.push(r.album.clone());
        }
    }
    out
}

fn clean(name: &'static str, note: &'static str, bright: f32, post: Vec<Slot>) -> Part {
    part(
        name,
        note,
        0.0,
        vec![comp()],
        amp(AmpModel::Clean, [3.0, 5.0, 6.0, 6.0, 5.0, 5.0, 5.0, bright]),
        open_cab(),
        post,
    )
}

fn clean_post() -> Vec<Slot> {
    vec![
        chorus(0.6, 0.4, 0.35),
        delay(480.0, 0.33, 0.22, PING_PONG),
        verb(HALL, 2.8, 0.22),
    ]
}

fn acoustic_part(name: &'static str, note: &'static str, post: Vec<Slot>) -> Part {
    part(
        name,
        note,
        0.0,
        vec![acoustic()],
        amp(AmpModel::Clean, [2.0, 5.0, 5.0, 6.0, 5.0, 5.0, 5.0, 0.0]),
        no_cab(),
        post,
    )
}

fn rhythm(
    name: &'static str,
    note: &'static str,
    gate: f32,
    drive: f32,
    model: AmpModel,
    v: [f32; 8],
    width: f32,
) -> Part {
    part(
        name,
        note,
        gate,
        vec![od(drive, 6.0, 8.0)],
        amp(model, v),
        v30(),
        vec![doubler(width)],
    )
}

fn lead(
    name: &'static str,
    note: &'static str,
    pre: Vec<Slot>,
    model: AmpModel,
    v: [f32; 8],
    post: Vec<Slot>,
) -> Part {
    part(name, note, -58.0, pre, amp(model, v), heavy_cab(), post)
}

fn bank() -> Vec<Rig> {
    use AmpModel::*;
    let mut out = Vec::new();
    let e_std = "E standard.";
    out.extend(album(
        BANDS[0],
        "Ride the Lightning",
        vec![
            acoustic_part(
                "Fight Fire with Fire Intro",
                "Acoustic intro. Use the neck pickup and play softly.",
                vec![chorus(0.5, 0.3, 0.25), verb(HALL, 2.2, 0.25)],
            ),
            rhythm(
                "Fight Fire with Fire",
                "Fast thrash riffs. E standard. The gate closes fast for tight stops.",
                -48.0,
                1.0,
                Brit,
                [8.0, 5.5, 5.0, 6.5, 6.0, 5.0, 5.5, 0.0],
                0.7,
            ),
            rhythm(
                "Ride the Lightning",
                e_std,
                -52.0,
                2.0,
                Brit,
                [7.5, 5.5, 5.5, 6.0, 6.0, 5.0, 5.0, 0.0],
                0.7,
            ),
            rhythm(
                "For Whom the Bell Tolls",
                "Less gain and more mids for the big chords.",
                -55.0,
                1.0,
                Brit,
                [6.0, 6.0, 6.5, 5.5, 5.5, 6.0, 5.5, 0.0],
                0.6,
            ),
            clean(
                "Fade to Black Clean",
                "Clean arpeggios. Use the neck pickup.",
                1.0,
                vec![
                    chorus(0.6, 0.4, 0.4),
                    delay(450.0, 0.3, 0.2, DIGITAL),
                    verb(HALL, 2.5, 0.2),
                ],
            ),
            lead(
                "Fade to Black Solo",
                "Singing lead with a long delay.",
                vec![od(3.0, 6.0, 7.0)],
                Brit,
                [7.5, 5.0, 7.0, 6.0, 6.0, 5.0, 5.0, 0.0],
                vec![delay(420.0, 0.3, 0.22, ANALOG), verb(HALL, 2.4, 0.2)],
            ),
            rhythm(
                "Creeping Death",
                e_std,
                -50.0,
                1.5,
                Brit,
                [7.5, 5.5, 5.0, 6.5, 6.0, 5.0, 5.0, 0.0],
                0.75,
            ),
            part(
                "The Call of Ktulu",
                "Arpeggio intro. Turn GAIN up for the heavy part.",
                0.0,
                vec![],
                amp(Brit, [3.5, 5.0, 6.0, 6.0, 5.0, 5.0, 5.0, 1.0]),
                v30(),
                vec![chorus(0.4, 0.3, 0.2), verb(PLATE, 1.8, 0.18)],
            ),
            lead(
                "Kirk Lead RTL",
                "Wah lead. Move the PEDAL knob or use an expression pedal on CC 11.",
                vec![wah(0.6), od(2.0, 6.0, 6.0)],
                Brit,
                [8.0, 5.0, 6.5, 6.0, 6.0, 5.0, 5.0, 0.0],
                vec![delay(380.0, 0.3, 0.2, ANALOG), verb(PLATE, 1.6, 0.15)],
            ),
        ],
    ));
    out.extend(album(
        BANDS[0],
        "Master of Puppets",
        vec![
            part(
                "Puppets Rhythm",
                "Scooped Mark tone with a V curve on the EQ.",
                -50.0,
                vec![],
                amp(Mark, [7.0, 5.0, 4.0, 7.0, 6.0, 6.0, 5.0, 0.0]),
                cab(CabModel::V30, 0.3, 70.0, 9000.0),
                vec![v_curve(1.0), doubler(0.7)],
            ),
            clean(
                "Puppets Interlude Clean",
                "The clean middle section.",
                1.0,
                vec![
                    chorus(0.6, 0.4, 0.35),
                    delay(400.0, 0.25, 0.18, DIGITAL),
                    verb(HALL, 2.4, 0.2),
                ],
            ),
            lead(
                "Kirk Lead Puppets",
                "Wah lead. Move the PEDAL knob or use CC 11.",
                vec![wah(0.6), od(2.0, 6.0, 6.0)],
                Mark,
                [8.0, 5.0, 6.0, 6.0, 6.0, 5.0, 5.0, 0.0],
                vec![delay(380.0, 0.3, 0.2, ANALOG), verb(PLATE, 1.6, 0.15)],
            ),
        ],
    ));
    out.extend(album(
        BANDS[0],
        "Metallica (Black Album)",
        vec![rhythm(
            "Black Album Rhythm",
            "Big and thick rhythm.",
            -55.0,
            1.0,
            Recto,
            [6.5, 6.0, 4.0, 6.0, 6.0, 6.0, 5.0, 0.0],
            0.8,
        )],
    ));
    let dm = "E standard. More mids and less scoop than the 80s albums.";
    out.extend(album(
        BANDS[0],
        "Death Magnetic",
        vec![
            rhythm(
                "That Was Just Your Life",
                dm,
                -55.0,
                0.5,
                Uber,
                [6.5, 5.5, 6.0, 6.0, 6.0, 5.5, 5.0, 0.0],
                0.8,
            ),
            rhythm(
                "The End of the Line",
                dm,
                -55.0,
                0.5,
                Uber,
                [7.0, 5.5, 6.0, 6.0, 6.0, 5.5, 5.0, 0.0],
                0.8,
            ),
            rhythm(
                "Broken, Beat & Scarred",
                dm,
                -55.0,
                1.0,
                Recto,
                [6.5, 6.0, 5.5, 6.0, 6.0, 6.0, 5.0, 0.0],
                0.8,
            ),
            clean(
                "The Day That Never Comes Clean",
                "Clean intro and verses.",
                0.0,
                clean_post(),
            ),
            rhythm(
                "The Day That Never Comes Heavy",
                "The fast heavy section at the end.",
                -55.0,
                0.5,
                Uber,
                [7.0, 5.5, 6.0, 6.0, 6.0, 5.5, 5.0, 0.0],
                0.8,
            ),
            rhythm(
                "All Nightmare Long",
                dm,
                -52.0,
                0.5,
                Uber,
                [7.0, 5.5, 5.5, 6.5, 6.0, 6.0, 5.0, 0.0],
                0.8,
            ),
            rhythm(
                "Cyanide",
                dm,
                -55.0,
                1.0,
                Recto,
                [6.5, 6.0, 5.5, 6.0, 6.0, 6.0, 5.0, 0.0],
                0.8,
            ),
            lead(
                "Suicide & Redemption Lead",
                "Long instrumental leads.",
                vec![od(2.0, 6.0, 7.0)],
                Uber,
                [8.0, 5.0, 6.5, 6.0, 6.0, 5.0, 5.0, 0.0],
                vec![delay(420.0, 0.35, 0.25, PING_PONG), verb(HALL, 2.6, 0.2)],
            ),
            lead(
                "Kirk Lead DM",
                "Heavy wah. Use an expression pedal on CC 11.",
                vec![wah(0.65), od(2.0, 6.0, 6.0)],
                Uber,
                [8.0, 5.0, 7.0, 6.0, 6.0, 5.0, 5.0, 0.0],
                vec![delay(380.0, 0.3, 0.2, ANALOG), verb(PLATE, 1.8, 0.15)],
            ),
        ],
    ));
    let tot = "Tight Mark tone. Petrucci played a 7-string on many songs.";
    let tot_rhythm = |name: &'static str, gain: f32, gate: f32, depth: f32| {
        part(
            name,
            tot,
            gate,
            vec![od(0.5, 6.0, 8.0)],
            amp(Mark, [gain, 5.0, 4.5, 6.5, 6.0, 6.5, 5.0, 0.0]),
            cab(CabModel::V30, 0.35, 85.0, 9000.0),
            vec![v_curve(depth), doubler(0.75)],
        )
    };
    out.extend(album(
        BANDS[1],
        "Train of Thought",
        vec![
            tot_rhythm("As I Am", 7.0, -52.0, 1.0),
            tot_rhythm("This Dying Soul", 7.5, -50.0, 1.0),
            clean(
                "Endless Sacrifice Clean",
                "Clean verses.",
                0.0,
                clean_post(),
            ),
            tot_rhythm("Endless Sacrifice Heavy", 7.0, -55.0, 0.9),
            tot_rhythm("Honor Thy Father", 8.0, -48.0, 1.1),
            tot_rhythm("Stream of Consciousness", 7.0, -55.0, 0.9),
            tot_rhythm("In the Name of God", 7.5, -52.0, 1.0),
            lead(
                "Petrucci Lead ToT",
                "Fast legato and shred. Smooth and compressed.",
                vec![od(1.0, 6.0, 8.0)],
                Mark,
                [8.0, 4.5, 6.5, 6.0, 6.0, 5.0, 5.0, 0.0],
                vec![delay(440.0, 0.35, 0.25, PING_PONG), verb(PLATE, 2.4, 0.2)],
            ),
        ],
    ));
    out.extend(album(
        BANDS[1],
        "Signature Tones",
        vec![
            part(
                "Petrucci Rhythm",
                "All-round Mark rhythm.",
                -55.0,
                vec![od(0.5, 6.0, 8.0)],
                amp(Mark, [6.5, 4.5, 5.0, 6.5, 6.0, 6.0, 5.0, 0.0]),
                cab(CabModel::V30, 0.35, 75.0, 9000.0),
                vec![v_curve(0.9), doubler(0.7)],
            ),
            lead(
                "Petrucci Lead",
                "All-round lead.",
                vec![od(1.0, 6.0, 8.0)],
                Mark,
                [7.5, 4.5, 6.5, 6.0, 6.0, 5.0, 5.0, 0.0],
                vec![delay(420.0, 0.35, 0.25, PING_PONG), verb(PLATE, 2.2, 0.2)],
            ),
            clean(
                "Petrucci Clean",
                "Wide chorus clean.",
                0.0,
                vec![
                    chorus(0.8, 0.5, 0.5),
                    delay(500.0, 0.35, 0.25, PING_PONG),
                    verb(HALL, 3.0, 0.25),
                ],
            ),
        ],
    ));
    out.extend(album(
        BANDS[2],
        "Symbolic",
        vec![
            part(
                "Symbolic Rhythm",
                "Boosted British crunch with clear mids.",
                -52.0,
                vec![od(3.0, 5.0, 8.0)],
                amp(Brit, [7.0, 6.0, 6.0, 6.0, 6.0, 5.0, 5.0, 0.0]),
                v30(),
                vec![doubler(0.6)],
            ),
            lead(
                "Chuck Lead",
                "Melodic lead with a short delay.",
                vec![od(4.0, 6.0, 8.0)],
                Brit,
                [8.0, 5.0, 7.0, 6.0, 6.0, 5.0, 5.0, 0.0],
                vec![delay(350.0, 0.3, 0.2, ANALOG), verb(HALL, 2.0, 0.18)],
            ),
        ],
    ));
    let a7x = |name: &'static str, model: AmpModel, mid: f32, note: &'static str| {
        rhythm(
            name,
            note,
            -55.0,
            0.5,
            model,
            [6.5, 6.0, mid, 6.0, 6.0, 6.0, 5.0, 0.0],
            0.85,
        )
    };
    let a7x_solo = |name: &'static str, twin: bool, note: &'static str| {
        let mut post = vec![delay(400.0, 0.4, 0.25, PING_PONG), verb(HALL, 2.8, 0.22)];
        if twin {
            post.insert(0, harmony(MAJOR_THIRD, 0.55));
        }
        lead(
            name,
            note,
            vec![od(1.0, 6.0, 8.0)],
            Uber,
            [7.5, 5.0, 6.5, 6.0, 6.0, 5.0, 5.0, 0.0],
            post,
        )
    };
    let twin =
        "Twin lead. HARMONY plays the second guitar a third up. Change INTERVAL to fit the key.";
    out.extend(album(
        BANDS[3],
        "City of Evil",
        vec![
            a7x(
                "Bat Country Rhythm",
                Recto,
                5.0,
                "Gallops and fast picking. Doubled guitars.",
            ),
            a7x_solo("Bat Country Solo", true, twin),
            a7x("Beast and the Harlot", Recto, 5.0, "Tight thrash rhythm."),
            clean(
                "Seize the Day Clean",
                "Clean intro and verses.",
                0.0,
                clean_post(),
            ),
            a7x_solo(
                "Seize the Day Solo",
                false,
                "Melodic solo with a long delay.",
            ),
        ],
    ));
    out.extend(album(
        BANDS[3],
        "Avenged Sevenfold",
        vec![
            a7x("Critical Acclaim", Uber, 5.0, "Heavy rhythm."),
            a7x("Almost Easy", Uber, 5.5, "Big chorus rhythm."),
            a7x("Scream", Uber, 5.0, "Tight riffs."),
            a7x("Afterlife Rhythm", Recto, 5.0, "Fast rhythm."),
            a7x_solo("Afterlife Solo", true, twin),
            acoustic_part(
                "Gunslinger Acoustic",
                "Acoustic guitar sound for the verses.",
                vec![chorus(0.4, 0.25, 0.15), verb(ROOM, 1.2, 0.2)],
            ),
            clean(
                "Dear God Clean",
                "Country clean with a slapback echo.",
                1.0,
                vec![delay(120.0, 0.1, 0.25, DIGITAL), verb(ROOM, 1.0, 0.15)],
            ),
            a7x("Unbound", Uber, 5.0, "Rhythm."),
            a7x(
                "A Little Piece of Heaven",
                Recto,
                5.5,
                "Rhythm with room for the orchestra.",
            ),
        ],
    ));
    out.extend(album(
        BANDS[3],
        "Signature Tones",
        vec![
            a7x("A7X Rhythm", Uber, 5.0, "All-round rhythm."),
            a7x_solo("Synyster Lead", false, "All-round lead."),
        ],
    ));
    let tyrant = "C standard. Turn on TUNE to play it with a guitar in E standard.";
    let ae = |name: &'static str, gain: f32, mid: f32, gate: f32| {
        part(
            name,
            tyrant,
            gate,
            vec![tune(-4.0), od(0.5, 5.5, 8.0)],
            amp(Block, [gain, 6.0, mid, 6.0, 6.0, 6.0, 5.0, 0.0]),
            cab(CabModel::V30, 0.3, 75.0, 9000.0),
            vec![doubler(0.8)],
        )
    };
    let ae_lead = |name: &'static str, note: &'static str, post: Vec<Slot>| {
        lead(
            name,
            note,
            vec![tune(-4.0), od(2.0, 6.0, 8.0)],
            Block,
            [7.5, 5.0, 7.0, 6.0, 5.5, 5.0, 5.0, 0.0],
            post,
        )
    };
    let mut vultures = clean("Vultures Clean", "Clean intro.", 0.0, clean_post());
    vultures.pre.insert(0, tune(-4.0));
    out.extend(album(
        BANDS[4],
        "Rise of the Tyrant",
        vec![
            ae("Blood on Your Hands", 6.5, 5.5, -55.0),
            ae("The Last Enemy", 7.0, 5.5, -55.0),
            ae("I Will Live Again", 6.5, 6.0, -55.0),
            ae("Night Falls Fast", 7.0, 5.0, -50.0),
            ae("Rise of the Tyrant", 6.5, 5.5, -55.0),
            vultures,
            ae_lead(
                "Vultures Solo",
                "Slow singing lead.",
                vec![delay(450.0, 0.4, 0.28, ANALOG), verb(HALL, 3.0, 0.22)],
            ),
            ae_lead(
                "Amott Twin Lead",
                "Twin lead. HARMONY plays a minor third up. Change INTERVAL to fit the key.",
                vec![
                    harmony(MINOR_THIRD, 0.6),
                    delay(400.0, 0.4, 0.25, ANALOG),
                    verb(HALL, 2.5, 0.2),
                ],
            ),
        ],
    ));
    out.extend(album(
        BANDS[4],
        "Signature Tones",
        vec![
            rhythm(
                "Amott Rhythm",
                "All-round rhythm.",
                -55.0,
                0.5,
                Block,
                [6.5, 6.0, 5.5, 6.0, 6.0, 6.0, 5.0, 0.0],
                0.8,
            ),
            lead(
                "Amott Lead",
                "All-round lead.",
                vec![od(2.0, 6.0, 8.0)],
                Block,
                [7.5, 5.0, 7.0, 6.0, 5.5, 5.0, 5.0, 0.0],
                vec![delay(400.0, 0.4, 0.25, ANALOG), verb(HALL, 2.5, 0.2)],
            ),
        ],
    ));
    let ajfa = "Very scooped and dry. Tight and hard picking. E standard.";
    let justice = |name: &'static str, gain: f32, depth: f32| {
        part(
            name,
            ajfa,
            -48.0,
            vec![od(0.5, 6.5, 8.0)],
            amp(Mark, [gain, 4.5, 3.5, 7.0, 6.5, 6.0, 5.0, 0.0]),
            cab(CabModel::V30, 0.25, 90.0, 8500.0),
            vec![v_curve(depth), doubler(0.75)],
        )
    };
    out.extend(album(
        BANDS[0],
        "...And Justice for All",
        vec![
            justice("Blackened", 7.5, 1.4),
            justice("...And Justice for All", 7.0, 1.4),
            justice("Eye of the Beholder", 7.0, 1.3),
            clean(
                "One Clean",
                "Clean intro and verses. Use the neck pickup.",
                1.0,
                vec![
                    chorus(0.5, 0.35, 0.3),
                    delay(420.0, 0.25, 0.18, DIGITAL),
                    verb(HALL, 2.4, 0.2),
                ],
            ),
            justice("One Heavy", 7.5, 1.4),
            lead(
                "One Solo",
                "Fast lead over the machine gun section.",
                vec![od(2.0, 6.0, 7.0)],
                Mark,
                [8.0, 4.5, 6.0, 6.5, 6.0, 5.0, 5.0, 0.0],
                vec![delay(360.0, 0.25, 0.18, ANALOG), verb(PLATE, 1.6, 0.12)],
            ),
            justice("The Shortest Straw", 7.0, 1.3),
            justice("Harvester of Sorrow", 6.5, 1.2),
            justice("The Frayed Ends of Sanity", 7.0, 1.3),
            clean(
                "To Live Is to Die Clean",
                "Clean arpeggios with a slow chorus.",
                1.0,
                vec![
                    chorus(0.4, 0.45, 0.35),
                    delay(500.0, 0.3, 0.2, DIGITAL),
                    verb(HALL, 3.0, 0.25),
                ],
            ),
            justice("Dyers Eve", 8.0, 1.4),
            lead(
                "Kirk Lead AJFA",
                "Wah lead. Move the PEDAL knob or use CC 11.",
                vec![wah(0.6), od(2.0, 6.0, 6.0)],
                Mark,
                [8.0, 4.5, 6.0, 6.5, 6.0, 5.0, 5.0, 0.0],
                vec![delay(360.0, 0.25, 0.15, ANALOG), verb(PLATE, 1.5, 0.12)],
            ),
        ],
    ));
    let anthems = "C standard. Turn on TUNE to play it with a guitar in E standard.";
    let anthem = |name: &'static str, gain: f32, mid: f32, gate: f32| {
        part(
            name,
            anthems,
            gate,
            vec![tune(-4.0), od(1.0, 5.5, 8.0)],
            amp(Block, [gain, 6.0, mid, 6.0, 6.0, 6.0, 5.0, 0.0]),
            cab(CabModel::V30, 0.3, 75.0, 8800.0),
            vec![doubler(0.8)],
        )
    };
    let anthem_lead = |name: &'static str, note: &'static str, post: Vec<Slot>| {
        lead(
            name,
            note,
            vec![tune(-4.0), od(2.5, 6.0, 8.0)],
            Block,
            [7.5, 5.0, 7.0, 6.0, 5.5, 5.0, 5.0, 0.0],
            post,
        )
    };
    out.extend(album(
        BANDS[4],
        "Anthems of Rebellion",
        vec![
            anthem("Silent Wars", 7.0, 5.5, -52.0),
            anthem("We Will Rise", 6.5, 6.0, -55.0),
            anthem("Dead Bury Their Dead", 7.0, 5.5, -52.0),
            anthem("End of the Line", 6.5, 6.0, -55.0),
            anthem("Dehumanization", 7.0, 5.0, -50.0),
            anthem("Leader of the Rats", 6.5, 6.0, -55.0),
            anthem_lead(
                "We Will Rise Solo",
                "Twin lead. HARMONY plays a minor third up. Change INTERVAL to fit the key.",
                vec![
                    harmony(MINOR_THIRD, 0.6),
                    delay(420.0, 0.35, 0.22, ANALOG),
                    verb(HALL, 2.4, 0.2),
                ],
            ),
            anthem_lead(
                "Marching on a Dead End Road",
                "Instrumental melody lead.",
                vec![delay(450.0, 0.4, 0.25, ANALOG), verb(HALL, 2.8, 0.22)],
            ),
        ],
    ));
    let core = "Drop C (C G C F A D). With a guitar in drop D, turn on TUNE.";
    let mut tight = rhythm(
        "Breakdown Chug",
        "Very tight low end for palm-muted breakdowns. Drop C.",
        -45.0,
        0.0,
        Block,
        [6.5, 5.5, 5.5, 6.5, 6.5, 5.0, 5.0, 0.0],
        0.9,
    );
    tight.pre.insert(0, tune(-2.0));
    tight.cab = cab(CabModel::V30, 0.3, 100.0, 9000.0);
    let core_rhythm = |name: &'static str, model: AmpModel, gain: f32, mid: f32| {
        part(
            name,
            core,
            -50.0,
            vec![tune(-2.0), od(0.0, 6.5, 9.0)],
            amp(model, [gain, 5.5, mid, 6.5, 6.5, 5.5, 5.0, 0.0]),
            cab(CabModel::V30, 0.3, 95.0, 9000.0),
            vec![doubler(0.9)],
        )
    };
    let mut core_clean = clean(
        "Clean Arpeggios",
        "Clean verses and intros with a wide chorus.",
        0.0,
        clean_post(),
    );
    core_clean.pre.insert(0, tune(-2.0));
    out.extend(album(
        BANDS[5],
        "Killswitch Style",
        vec![
            core_rhythm("Verse Riffs", Recto, 6.5, 5.5),
            core_rhythm("Big Chorus", Recto, 6.0, 6.0),
            tight,
            core_clean,
            part(
                "Melodic Lead",
                "Melodic lead with a harmony a third up. Change INTERVAL to fit the key.",
                -58.0,
                vec![tune(-2.0), od(1.0, 6.0, 8.0)],
                amp(Uber, [7.0, 5.0, 6.5, 6.0, 6.0, 5.0, 5.0, 0.0]),
                heavy_cab(),
                vec![
                    harmony(MAJOR_THIRD, 0.5),
                    delay(380.0, 0.35, 0.22, PING_PONG),
                    verb(HALL, 2.4, 0.2),
                ],
            ),
        ],
    ));
    out.extend(album(
        BANDS[5],
        "Modern Toolkit",
        vec![
            core_rhythm("Modern Rhythm 5150 Style", Block, 6.5, 5.5),
            core_rhythm("Modern Rhythm Recto Style", Recto, 6.5, 5.0),
            part(
                "Ambient Clean",
                "Clean with long delay and reverb for intros.",
                0.0,
                vec![comp()],
                amp(Clean, [3.0, 5.0, 5.5, 6.0, 5.0, 5.0, 5.0, 0.0]),
                open_cab(),
                vec![
                    chorus(0.4, 0.35, 0.3),
                    delay(520.0, 0.45, 0.3, PING_PONG),
                    verb(HALL, 5.0, 0.35),
                ],
            ),
        ],
    ));
    out.extend(album(
        BANDS[6],
        "Extras",
        vec![
            part(
                "Swedish Death",
                "HM-2 chainsaw tone.",
                -50.0,
                vec![Slot::with(
                    Kind::Swedish,
                    &[("LEVEL", 6.0), ("LOW", 10.0), ("HIGH", 10.0), ("DIST", 9.0)],
                )],
                amp(Brit, [3.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 0.0]),
                cab(CabModel::Greenback, 0.3, 60.0, 8000.0),
                vec![],
            ),
            part(
                "Modern Djent",
                "Very tight low end.",
                -50.0,
                vec![od(0.0, 6.0, 9.0)],
                amp(Block, [6.0, 5.0, 5.0, 6.5, 6.5, 5.0, 5.0, 0.0]),
                cab(CabModel::V30, 0.3, 90.0, 9500.0),
                vec![doubler(0.9)],
            ),
            part(
                "Clean Room",
                "Plain clean amp in a room.",
                0.0,
                vec![],
                amp(Clean, [3.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 0.0]),
                open_cab(),
                vec![verb(ROOM, 0.8, 0.2)],
            ),
        ],
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Chain;
    use crate::library::Library;
    use std::f32::consts::TAU;

    const RATE: f32 = 48000.0;

    fn riff(i: usize) -> f32 {
        let t = i as f32 / RATE;
        let gate = if (i / 6000).is_multiple_of(2) {
            1.0
        } else {
            0.3
        };
        gate * 0.15
            * ((t * 82.4 * TAU).sin() + (t * 123.5 * TAU).sin() + 0.4 * (t * 247.0 * TAU).sin())
    }

    fn level(rig: &Rig) -> f32 {
        let mut c = Chain::build(rig, RATE, &mut Library::default());
        let mut sum = 0.0;
        let n = 48000;
        for i in 0..n {
            let x = riff(i) * c.input_gain();
            let [l, r] = c.process(x);
            assert!(
                l.is_finite() && r.is_finite() && l.abs() <= 1.0,
                "{}",
                rig.name
            );
            if i > n / 2 {
                sum += l * l + r * r;
            }
        }
        (sum / n as f32).sqrt()
    }

    mod when_loading_the_factory_bank {
        use super::*;

        #[test]
        fn covers_every_band_with_unique_names() {
            let bank = factory();
            for band in BANDS {
                assert!(
                    bank.iter().filter(|r| r.category == band).count() >= 2,
                    "{band}"
                );
            }
            for (i, r) in bank.iter().enumerate() {
                assert!(
                    !bank[i + 1..].iter().any(|o| o.name == r.name),
                    "{}",
                    r.name
                );
            }
        }

        #[test]
        fn plays_every_preset_at_a_similar_level() {
            let levels: Vec<(String, f32)> = factory()
                .iter()
                .map(|r| (r.name.clone(), level(r)))
                .collect();
            let loudest = levels.iter().map(|l| l.1).fold(0.0, f32::max);
            for (name, l) in &levels {
                assert!(*l > loudest / 2.0, "{name} {l} vs {loudest}");
            }
        }

        #[test]
        fn groups_every_preset_by_album_with_a_note() {
            let bank = factory();
            for r in &bank {
                assert!(!r.album.is_empty(), "{}", r.name);
                assert!(!r.note.is_empty(), "{}", r.name);
            }
            let metallica = albums(&bank, BANDS[0]);
            assert!(metallica.contains(&"Ride the Lightning".to_string()));
            assert!(metallica.contains(&"Death Magnetic".to_string()));
            assert!(albums(&bank, BANDS[1]).contains(&"Train of Thought".to_string()));
            assert!(albums(&bank, BANDS[3]).contains(&"City of Evil".to_string()));
            assert!(albums(&bank, BANDS[4]).contains(&"Rise of the Tyrant".to_string()));
        }

        #[test]
        fn survives_a_json_round_trip() {
            for r in factory() {
                let back: Rig = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
                assert_eq!(back, r);
            }
        }

        #[test]
        fn keeps_every_value_inside_its_range() {
            for r in factory() {
                let mut fixed = r.clone();
                fixed.fix();
                assert_eq!(fixed, r, "{}", r.name);
            }
        }
    }
}
