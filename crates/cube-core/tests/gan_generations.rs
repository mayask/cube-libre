//! Synthetic fixtures only: no recorded user MACs, device names or packets.
//! Layouts cross-checked against the credited upstream drivers; AES vectors
//! independently generated using OpenSSL, not our encrypt implementation.
use cube_core::{
    CubeState, Face, GanCipher, Move,
    cube::Cubies,
    protocol::{Decoder, Event, Generation, KeyProfile, ReadRequest, gen1},
    sync::{ObservedMove, Tracker},
};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn put(data: &mut [u8], offset: usize, width: usize, value: u32) {
    for i in 0..width {
        let bit = offset + i;
        let mask = 1 << (7 - bit % 8);
        data[bit / 8] =
            (data[bit / 8] & !mask) | ((((value >> (width - 1 - i)) & 1) as u8) << (7 - bit % 8));
    }
}
fn snapshot(g: Generation, counter: u8, c: &Cubies) -> Vec<u8> {
    let mut data = vec![0; 20];
    let (cp, co, ep, eo) = match g {
        Generation::Gen2 => {
            put(&mut data, 0, 4, 4);
            put(&mut data, 4, 8, counter.into());
            (12, 33, 47, 91)
        }
        Generation::Gen3 => {
            data[..5].copy_from_slice(&[0x55, 2, 14, counter, 0x12]);
            (40, 61, 77, 121)
        }
        Generation::Gen4 => {
            data[..4].copy_from_slice(&[0xed, 14, counter, 0x12]);
            (32, 53, 69, 113)
        }
        Generation::Gen1 => unreachable!(),
    };
    for i in 0..7 {
        put(&mut data, cp + i * 3, 3, c.cp[i].into());
        put(&mut data, co + i * 2, 2, c.co[i].into());
    }
    for i in 0..11 {
        put(&mut data, ep + i * 4, 4, c.ep[i].into());
        put(&mut data, eo + i, 1, c.eo[i].into());
    }
    data
}
fn gen2_moves(counter: u8, newest_first: &[Move]) -> Vec<u8> {
    let mut data = vec![0; 20];
    put(&mut data, 0, 4, 2);
    put(&mut data, 4, 8, counter.into());
    for (i, movement) in newest_first.iter().enumerate() {
        put(&mut data, 12 + i * 5, 4, movement.face as u32);
        put(&mut data, 16 + i * 5, 1, u32::from(movement.turns == 3));
        put(&mut data, 47 + i * 16, 16, 25);
    }
    data
}
fn gen1_colors(cube: &CubeState) -> Vec<u8> {
    let mut data = vec![0; 20];
    for face in 0..6 {
        let mut packed = 0;
        for index in (0..9).filter(|&i| i != 4) {
            packed = (packed << 3) | cube.stickers()[face * 9 + index] as u32;
        }
        let i = face * 3;
        data[i ^ 1] = (packed >> 16) as u8;
        data[(i + 1) ^ 1] = (packed >> 8) as u8;
        data[(i + 2) ^ 1] = packed as u8;
    }
    data
}
fn gen1_state(counter: u8, newest_first: &[Move]) -> Vec<u8> {
    let mut data = vec![0; 20];
    data[12] = counter;
    for (i, movement) in newest_first.iter().enumerate() {
        data[18 - i] = movement.face as u8 * 3 + movement.turns - 1;
    }
    data
}
fn feed(tracker: &mut Tracker, events: Vec<Event>) {
    for event in events {
        match event {
            Event::Move {
                counter,
                cube_time_ms,
                movement,
            } => {
                tracker.on_move(ObservedMove {
                    counter,
                    cube_time_ms,
                    movement,
                });
            }
            Event::Snapshot { counter, state } => {
                tracker.on_snapshot(counter, state);
            }
            _ => {}
        }
    }
}

#[test]
fn discovery_selects_by_service_not_name_and_scopes_shared_characteristics() {
    for g in Generation::ALL {
        assert_eq!(
            Generation::from_services(&["unrelated", &g.service().to_uppercase()]),
            Some(g)
        );
    }
    assert_eq!(
        Generation::from_services(&[gen1::SERVICE, Generation::Gen4.service()]),
        Some(Generation::Gen4)
    );
    assert_eq!(Generation::from_services(&["GANicE3"]), None);
    assert_eq!(Generation::Gen1.notification_profile(), None);
    assert_eq!(
        Generation::Gen4.notification_profile().unwrap().0,
        gen1::STATE
    );
    assert_ne!(Generation::Gen4.service(), gen1::SERVICE);
}
#[test]
fn every_outbound_command_is_a_generation_specific_read_request() {
    let cipher = GanCipher::new("01:02:03:04:05:06".parse().unwrap());
    for (g, prefixes) in [
        (Generation::Gen2, vec![vec![4], vec![9], vec![5]]),
        (
            Generation::Gen3,
            vec![vec![0x68, 1], vec![0x68, 7], vec![0x68, 4]],
        ),
        (
            Generation::Gen4,
            vec![
                vec![0xdd, 4, 0, 0xed, 0, 0],
                vec![0xdd, 4, 0, 0xef, 0, 0],
                vec![0xdf, 3, 0, 0, 0],
            ],
        ),
    ] {
        for (request, prefix) in [
            ReadRequest::State,
            ReadRequest::Battery,
            ReadRequest::Hardware,
        ]
        .into_iter()
        .zip(prefixes)
        {
            let mut expected = vec![0; if g == Generation::Gen3 { 16 } else { 20 }];
            expected[..prefix.len()].copy_from_slice(&prefix);
            assert_eq!(
                cipher
                    .decrypt(&cipher.request(g, request).unwrap())
                    .unwrap(),
                expected
            );
            assert!(cipher.request(Generation::Gen1, request).is_err());
        }
    }
}
#[test]
fn openssl_vectors_cover_gen2_gen3_and_the_aicube_key_variant() {
    let mac = "01:02:03:04:05:06".parse().unwrap();
    for (profile, g, expected) in [
        (
            KeyProfile::Gan,
            Generation::Gen2,
            "912f3b3360fc1d67f61750b22d936cbb666076dc",
        ),
        (
            KeyProfile::Gan,
            Generation::Gen3,
            "ef86a8ef7c37b79d1b320caaa10d601c",
        ),
        (
            KeyProfile::MoyuAi2023,
            Generation::Gen2,
            "a1cfd311559f1186b8a46d8dd3f924a10085ea49",
        ),
    ] {
        assert_eq!(
            GanCipher::for_profile(mac, profile)
                .request(g, ReadRequest::State)
                .unwrap(),
            hex(expected)
        );
    }
}
#[test]
fn openssl_vectors_cover_both_gen1_firmware_keys_and_ecb_overlap() {
    for (firmware, vector) in [
        ([1, 0, 8], "30d0e468367b4bc617a839a83a5cdfc8b576e0ec"),
        ([1, 1, 0], "9f121abd5803fa4602cc5419a0680264e54a9c5a"),
    ] {
        let cipher = gen1::Cipher::new(&firmware, &[1, 2, 3, 4, 5, 6]).unwrap();
        assert_eq!(
            cipher.decrypt(&hex(vector)).unwrap(),
            (0..20).collect::<Vec<u8>>()
        );
        assert!(cipher.decrypt(&[0; 15]).is_err());
    }
    for firmware in [
        vec![],
        vec![1, 0],
        vec![1, 0, 7],
        vec![1, 2, 0],
        vec![2, 0, 8],
    ] {
        assert!(gen1::Cipher::new(&firmware, &[1; 6]).is_err());
    }
    assert!(gen1::Cipher::new(&[1, 0, 8], &[1; 5]).is_err());
}
#[test]
fn all_compact_snapshot_layouts_agree_and_validate_full_cubie_invariants() {
    let mut c = Cubies::solved();
    c.cp.swap(0, 1);
    c.ep.swap(0, 1);
    c.co[0] = 1;
    c.co[1] = 2;
    c.eo[0] = 1;
    c.eo[1] = 1;
    let expected = CubeState::from_cubies(&c).unwrap();
    for g in [Generation::Gen2, Generation::Gen3, Generation::Gen4] {
        let mut decoder = Decoder::new(g);
        assert_eq!(
            decoder.decode(&snapshot(g, 255, &c)).unwrap(),
            vec![Event::Snapshot {
                counter: 255,
                state: expected.clone()
            }]
        );
        c.ep.swap(0, 1); // Mismatching corner/edge parity, though colors/counts look valid.
        assert!(decoder.decode(&snapshot(g, 255, &c)).is_err());
        c.ep.swap(0, 1);
    }
}
#[test]
fn every_face_and_direction_decodes_in_gen2_and_gen3() {
    for face in Face::ALL {
        for inverse in [false, true] {
            let movement = Move::quarter(face, inverse);
            let mut decoder = Decoder::new(Generation::Gen2);
            decoder
                .decode(&snapshot(Generation::Gen2, 10, &Cubies::solved()))
                .unwrap();
            assert_eq!(
                decoder.decode(&gen2_moves(11, &[movement])).unwrap(),
                vec![Event::Move {
                    counter: 11,
                    cube_time_ms: 25,
                    movement
                }]
            );
            let mut p = vec![0; 16];
            p[..10].copy_from_slice(&[
                0x55,
                1,
                7,
                0x78,
                0x56,
                0x34,
                0x12,
                255,
                0x12,
                [2, 32, 8, 1, 16, 4][face as usize] | (u8::from(inverse) << 6),
            ]);
            assert_eq!(
                Decoder::new(Generation::Gen3).decode(&p).unwrap(),
                vec![Event::Move {
                    counter: 255,
                    cube_time_ms: 0x12345678,
                    movement
                }]
            );
            p[9] |= 0x80;
            assert!(Decoder::new(Generation::Gen3).decode(&p).is_err());
        }
    }
}
#[test]
fn gen2_recovers_its_buffered_moves_in_order_across_wrap_and_deduplicates() {
    let mut decoder = Decoder::new(Generation::Gen2);
    let mut tracker = Tracker::default();
    let moves = ["R", "U", "F'"]; // Chronological order.
    let newest = moves
        .iter()
        .rev()
        .map(|m| m.parse().unwrap())
        .collect::<Vec<_>>();
    let packet = gen2_moves(1, &newest);
    assert!(decoder.decode(&packet).unwrap().is_empty()); // No fake initial solved state.
    feed(
        &mut tracker,
        decoder
            .decode(&snapshot(Generation::Gen2, 254, &Cubies::solved()))
            .unwrap(),
    );
    let events = decoder.decode(&packet).unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| match e {
                Event::Move { counter, .. } => *counter,
                _ => panic!(),
            })
            .collect::<Vec<_>>(),
        [255, 0, 1]
    );
    feed(&mut tracker, events);
    let mut expected = CubeState::solved();
    for text in moves {
        expected.apply(text.parse().unwrap());
    }
    assert!(tracker.synced);
    assert_eq!(tracker.state, Some(expected));
    assert!(decoder.decode(&packet).unwrap().is_empty());
    assert!(
        decoder
            .decode(&gen2_moves(0, &["L".parse().unwrap()]))
            .unwrap()
            .is_empty()
    );
    assert!(
        decoder
            .decode(&snapshot(Generation::Gen2, 255, &Cubies::solved()))
            .is_ok()
    );
    assert!(decoder.decode(&packet).unwrap().is_empty()); // Old snapshot cannot rewind decoder.
}
#[test]
fn gen2_gap_beyond_seven_moves_freezes_prediction_until_authoritative_snapshot() {
    let mut decoder = Decoder::new(Generation::Gen2);
    let mut tracker = Tracker::default();
    feed(
        &mut tracker,
        decoder
            .decode(&snapshot(Generation::Gen2, 10, &Cubies::solved()))
            .unwrap(),
    );
    feed(
        &mut tracker,
        decoder
            .decode(&gen2_moves(20, &["U".parse().unwrap(); 7]))
            .unwrap(),
    );
    assert!(!tracker.synced);
    assert!(tracker.state.as_ref().unwrap().is_solved());
    feed(
        &mut tracker,
        decoder
            .decode(&snapshot(Generation::Gen2, 20, &Cubies::solved()))
            .unwrap(),
    );
    assert!(tracker.synced);
    assert_eq!(tracker.counter, Some(20));
}
#[test]
fn gen2_invalid_move_window_does_not_advance_internal_counter() {
    let mut decoder = Decoder::new(Generation::Gen2);
    decoder
        .decode(&snapshot(Generation::Gen2, 10, &Cubies::solved()))
        .unwrap();
    let mut p = gen2_moves(11, &["R".parse().unwrap()]);
    put(&mut p, 12 + 5 * 6, 4, 7);
    assert!(decoder.decode(&p).is_err());
    assert_eq!(
        decoder
            .decode(&gen2_moves(11, &["R".parse().unwrap()]))
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn battery_hardware_and_disconnect_work_for_all_generations() {
    let mut g2 = Decoder::new(Generation::Gen2);
    let mut p = vec![0; 20];
    p[0] = 0x90;
    p[1] = 82;
    assert_eq!(g2.decode(&p).unwrap(), vec![Event::Battery(82)]);
    p[1] = 101;
    assert!(g2.decode(&p).is_err());
    p.fill(0);
    p[..5].copy_from_slice(&[0x50, 1, 2, 3, 4]);
    p[5..13].copy_from_slice(b"TESTCUBE");
    assert_eq!(
        g2.decode(&p).unwrap(),
        vec![
            Event::HardwareName("TESTCUBE".into()),
            Event::HardwareVersion("1.2".into()),
            Event::Firmware("3.4".into())
        ]
    );
    p[0] = 0xd0;
    assert_eq!(g2.decode(&p).unwrap(), vec![Event::Disconnect]);
    let mut g3 = Decoder::new(Generation::Gen3);
    assert_eq!(
        g3.decode(&[0x55, 0x10, 1, 82]).unwrap(),
        vec![Event::Battery(82)]
    );
    assert!(g3.decode(&[0x55, 0x10, 1, 101]).is_err());
    assert_eq!(
        g3.decode(&[0x55, 7, 8, 0, b'T', b'E', b'S', b'T', b'1', 0x34, 0x12])
            .unwrap(),
        vec![
            Event::HardwareName("TEST1".into()),
            Event::Firmware("3.4".into()),
            Event::HardwareVersion("1.2".into())
        ]
    );
    assert_eq!(
        g3.decode(&[0x55, 0x11, 1, 0]).unwrap(),
        vec![Event::Disconnect]
    );
    assert_eq!(
        gen1::battery(&[0, 0, 0, 0, 0, 0, 0, 82]).unwrap(),
        Event::Battery(82)
    );
    assert!(gen1::battery(&[0, 0, 0, 0, 0, 0, 0, 101]).is_err());
    assert_eq!(
        Decoder::new(Generation::Gen4)
            .decode(&[0xef, 1, 82])
            .unwrap(),
        vec![Event::Battery(82)]
    );
}
#[test]
fn gen1_decodes_every_face_with_clockwise_half_and_inverse_turns() {
    for face in Face::ALL {
        for turns in 1..=3 {
            let mut decoder = gen1::Decoder::default();
            let before = gen1_state(10, &[]);
            decoder
                .snapshot(&before, &gen1_colors(&CubeState::solved()), &before)
                .unwrap();
            let movement = Move { face, turns };
            let packet = gen1_state(11, &[movement]);
            assert_eq!(
                decoder.moves(&packet, &[0; 20]).unwrap(),
                vec![Event::Move {
                    counter: 11,
                    cube_time_ms: 0,
                    movement
                }]
            );
        }
    }
}
#[test]
fn gen1_facelets_round_trip_scrambles_and_reject_impossible_physical_states() {
    let mut cube = CubeState::solved();
    for text in ["R", "U", "F'", "D2", "B", "L'", "R2"] {
        cube.apply(text.parse().unwrap());
        assert_eq!(gen1::facelets(&gen1_colors(&cube)).unwrap(), cube);
        assert_eq!(
            CubeState::from_facelets(&cube.facelet_string()).unwrap(),
            cube
        );
    }
    let solved = CubeState::solved().facelet_string();
    let mut impossible = solved.into_bytes();
    impossible.swap(5, 10); // Single flipped UR edge.
    assert!(CubeState::from_facelets(std::str::from_utf8(&impossible).unwrap()).is_err());
    impossible.swap(5, 10);
    impossible[4] = b'R';
    assert!(CubeState::from_facelets(std::str::from_utf8(&impossible).unwrap()).is_err());
    assert!(CubeState::from_facelets("🧊").is_err());
    assert!(gen1::facelets(&[255; 20]).is_err());
}
#[test]
fn gen1_snapshot_never_invents_a_zero_counter_or_accepts_a_read_race() {
    let mut decoder = gen1::Decoder::default();
    let before = gen1_state(72, &[]);
    let after = gen1_state(73, &["R".parse().unwrap()]);
    let colors = gen1_colors(&CubeState::solved());
    assert!(
        decoder
            .snapshot(&before, &colors, &after)
            .unwrap()
            .is_none()
    );
    assert!(decoder.moves(&after, &[0; 20]).unwrap().is_empty());
    assert_eq!(
        decoder.snapshot(&after, &colors, &after).unwrap(),
        Some(Event::Snapshot {
            counter: 73,
            state: CubeState::solved()
        })
    );
}
#[test]
fn gen1_six_move_window_supports_half_turns_wrap_and_gap_repair() {
    let mut decoder = gen1::Decoder::default();
    let mut tracker = Tracker::default();
    let before = gen1_state(254, &[]);
    feed(
        &mut tracker,
        vec![
            decoder
                .snapshot(&before, &gen1_colors(&CubeState::solved()), &before)
                .unwrap()
                .unwrap(),
        ],
    );
    let newest = [
        "F'".parse().unwrap(),
        "U2".parse().unwrap(),
        "R".parse().unwrap(),
    ];
    let current = gen1_state(1, &newest);
    feed(&mut tracker, decoder.moves(&current, &[0; 20]).unwrap());
    let mut expected = CubeState::solved();
    for text in ["R", "U2", "F'"] {
        expected.apply(text.parse().unwrap());
    }
    assert!(tracker.synced);
    assert_eq!(tracker.state, Some(expected));
    assert!(decoder.moves(&current, &[0; 20]).unwrap().is_empty());
    let gap = gen1_state(10, &["L".parse().unwrap(); 6]);
    feed(&mut tracker, decoder.moves(&gap, &[0; 20]).unwrap());
    assert!(!tracker.synced);
    feed(
        &mut tracker,
        vec![
            decoder
                .snapshot(&gap, &gen1_colors(&CubeState::solved()), &gap)
                .unwrap()
                .unwrap(),
        ],
    );
    assert!(tracker.synced);
    assert!(tracker.state.unwrap().is_solved());
}
#[test]
fn gen3_reconciliation_wraps_and_repairs_gaps_without_fabricating_history() {
    let mut decoder = Decoder::new(Generation::Gen3);
    let mut tracker = Tracker::default();
    feed(
        &mut tracker,
        decoder
            .decode(&snapshot(Generation::Gen3, 254, &Cubies::solved()))
            .unwrap(),
    );
    for counter in [255, 0] {
        let mut p = vec![0; 16];
        p[..10].copy_from_slice(&[0x55, 1, 7, counter, 0, 0, 0, counter, 0x12, 32]);
        feed(&mut tracker, decoder.decode(&p).unwrap());
    }
    assert!(tracker.synced);
    assert_eq!(tracker.counter, Some(0));
    let mut p = vec![0; 16];
    p[..10].copy_from_slice(&[0x55, 1, 7, 4, 0, 0, 0, 4, 0x12, 8]);
    feed(&mut tracker, decoder.decode(&p).unwrap());
    assert!(!tracker.synced);
    feed(
        &mut tracker,
        decoder
            .decode(&snapshot(Generation::Gen3, 4, &Cubies::solved()))
            .unwrap(),
    );
    assert!(tracker.synced);
    assert_eq!(tracker.observed_turns, 3); // Missing turns are not invented in log.
}
#[test]
fn truncated_and_random_packets_never_panic_in_any_driver() {
    for size in 0..48 {
        for value in 0..=255u8 {
            let mut p = vec![value; size];
            if size > 2 {
                p[2] = size.saturating_sub(3) as u8;
            }
            for g in [Generation::Gen2, Generation::Gen3, Generation::Gen4] {
                let _ = Decoder::new(g).decode(&p);
            }
            let mut gen1 = gen1::Decoder::default();
            let _ = gen1.snapshot(&p, &p, &p);
            let _ = gen1.moves(&p, &p);
            let _ = gen1::battery(&p);
        }
    }
}
