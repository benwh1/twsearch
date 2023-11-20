use std::sync::Mutex;

use cubing::alg::{Alg, Pause};
use lazy_static::lazy_static;
use url::Url;

use crate::{
    _internal::{
        options::MetricEnum, AdditionalSolutionCondition, IDFSearch, IndividualSearchOptions,
        PackedKPattern, PackedKPuzzle, PackedKPuzzleOrbitInfo, SearchGenerators,
    },
    scramble::{
        puzzles::definitions::{
            cube4x4x4_packed_kpuzzle, cube4x4x4_phase1_target_pattern,
            cube4x4x4_phase2_target_pattern,
        },
        randomize::{
            basic_parity, randomize_orbit_naive, BasicParity, OrbitOrientationConstraint,
            OrbitPermutationConstraint,
        },
        scramble_search::{basic_idfs, idfs_with_target_pattern},
    },
};

use super::super::scramble_search::generators_from_vec_str;

const NUM_4X4X4_EDGES: usize = 24;

/**
 * Each pair of edges ("wings") on a solved 4x4x4 has two position:
 *
 * - The "high" position — this includes UBl (the first piece in Speffz) and all the places that the UBl piece can be moved by <U, L, R, D>
 * - The "low" position — the other position.
 *
 * Further:
 *
 * - A piece that starts in a high position is a high piece.
 * - A piece that starts in a high position is a low piece.
 *
 * These orbits are preserved by U, Uw2, D, Dw2, F, Fw2, B, Bw2, R2, Rw2, L2, and Lw2.
 *
 * And:
 *
 * Each high-low pair is assigned an index, which is the position index of the high position/piece in Speffz.
 *
 * This encodes the convention established by: http://cubezzz.dyndns.org/drupal/?q=node/view/73#comment-2588
 */
#[derive(Copy, Clone, PartialEq)]
struct EdgePairIndex(usize);
const EDGE_TO_INDEX: [EdgePairIndex; NUM_4X4X4_EDGES] = [
    // U
    EdgePairIndex(0), // high
    EdgePairIndex(1), // high
    EdgePairIndex(2), // high
    EdgePairIndex(3), // high
    // L
    EdgePairIndex(3),  // low
    EdgePairIndex(11), // low
    EdgePairIndex(23), // low
    EdgePairIndex(17), // low
    // F
    EdgePairIndex(2),  // low
    EdgePairIndex(9),  // high
    EdgePairIndex(20), // low
    EdgePairIndex(11), // high
    // R
    EdgePairIndex(1),  // low
    EdgePairIndex(19), // low
    EdgePairIndex(21), // low
    EdgePairIndex(9),  // low
    // B
    EdgePairIndex(0),  // low
    EdgePairIndex(17), // high
    EdgePairIndex(22), // low
    EdgePairIndex(19), // high
    // D
    EdgePairIndex(20), // high
    EdgePairIndex(21), // high
    EdgePairIndex(22), // high
    EdgePairIndex(23), // high
];

// Checks if either a position or a piece is high (same code for both).
fn is_high(position_or_piece: usize) -> bool {
    EDGE_TO_INDEX[position_or_piece].0 == position_or_piece
}

#[derive(Clone, PartialEq, Debug)]
enum Phase2EdgeOrientation {
    Unknown,
    // Either a high piece in a high position, or a low piece in a low position.
    Oriented,
    // Either a high piece in a low position, or a low piece in a high position.
    Misoriented,
}

pub struct Scramble4x4x4FourPhase {
    packed_kpuzzle: PackedKPuzzle,

    _filtering_idfs: IDFSearch,

    phase1_target_pattern: PackedKPattern,
    phase1_idfs: IDFSearch,

    phase2_center_target_pattern: PackedKPattern,
    phase2_idfs: IDFSearch,
}

impl Default for Scramble4x4x4FourPhase {
    fn default() -> Self {
        let packed_kpuzzle = cube4x4x4_packed_kpuzzle();

        let phase1_generators = generators_from_vec_str(vec![
            "Uw", "U", "Lw", "L", "Fw", "F", "Rw", "R", "Bw", "B", "Dw", "D",
        ]);
        // TODO: support normalizing orientation/ignoring orientation/24 targets, so that this checks for unoriented distance to solved.
        let filtering_idfs = basic_idfs(&packed_kpuzzle, phase1_generators.clone(), Some(32));

        let phase1_target_pattern = cube4x4x4_phase1_target_pattern();
        // dbg!(&phase1_target_pattern);
        let phase1_idfs = idfs_with_target_pattern(
            &packed_kpuzzle,
            phase1_generators.clone(),
            phase1_target_pattern.clone(),
            None,
        );

        let phase2_generators =
            generators_from_vec_str(vec!["Uw2", "U", "L", "F", "Rw", "R", "B", "Dw2", "D"]);
        let phase2_center_target_pattern = cube4x4x4_phase2_target_pattern();
        // dbg!(&phase2_center_target_pattern);
        let phase2_idfs = idfs_with_target_pattern(
            &packed_kpuzzle,
            phase2_generators.clone(),
            phase2_center_target_pattern.clone(),
            None,
        );

        Self {
            packed_kpuzzle,
            _filtering_idfs: filtering_idfs,
            phase1_target_pattern,
            phase1_idfs,
            phase2_center_target_pattern,
            phase2_idfs,
        }
    }
}

pub fn random_4x4x4_pattern(hardcoded_scramble_alg_for_testing: Option<&Alg>) -> PackedKPattern {
    dbg!("random_4x4x4_pattern");
    let packed_kpuzzle = cube4x4x4_packed_kpuzzle();
    let mut scramble_pattern = packed_kpuzzle.default_pattern();

    match hardcoded_scramble_alg_for_testing {
        Some(hardcoded_scramble_alg_for_testing) => {
            let transformation = packed_kpuzzle
                .transformation_from_alg(hardcoded_scramble_alg_for_testing)
                .unwrap();
            scramble_pattern = scramble_pattern.apply_transformation(&transformation);
        }
        None => {
            for orbit_info in &packed_kpuzzle.data.orbit_iteration_info {
                randomize_orbit_naive(
                    &mut scramble_pattern,
                    orbit_info,
                    OrbitPermutationConstraint::AnyPermutation,
                    OrbitOrientationConstraint::OrientationsMustSumToZero,
                );
            }
        }
    }
    scramble_pattern
}

const C8_4D2: usize = 35;
const C16_8: usize = 12870;
const PHASE2_MOVECOUNT: usize = 23;
const EDGE_PARITY: usize = 2;
const PHASE2PRUNE_SIZE: usize = C8_4D2 * C16_8 * EDGE_PARITY / 2;
const INF: usize = 1000000000; // larger than any symcoord

#[derive(Clone, Copy, Debug)]
enum CoordinateTable {
    Coord84,
    Coord168,
    Coordep,
}

trait Coord {
    fn coordinate_for_pattern(&self, pattern: &PackedKPattern) -> usize;
    fn main_table(&mut self) -> &mut [[usize; PHASE2_MOVECOUNT]];
}

struct Coord84 {
    pack84: [i32; 256],
    c84move: [[usize; PHASE2_MOVECOUNT]; C8_4D2],
}

impl Coord for Coord84 {
    fn coordinate_for_pattern(&self, pattern: &PackedKPattern) -> usize {
        let mut bits = 0;
        // TODO: store this in the struct?
        let centers_orbit_info = &pattern
            .packed_orbit_data
            .packed_kpuzzle
            .data
            .orbit_iteration_info[2];
        assert!(centers_orbit_info.name == "CENTERS".into());
        for idx in [4, 5, 6, 7, 12, 13, 14, 15] {
            bits *= 2;
            if pattern.get_piece_or_permutation(&centers_orbit_info, idx) == 1 {
                bits += 1
            }
        }
        self.pack84[bits] as usize
    }

    fn main_table(&mut self) -> &mut [[usize; PHASE2_MOVECOUNT]] {
        &mut self.c84move
    }
}

impl Default for Coord84 {
    fn default() -> Self {
        Self {
            pack84: [0; 256],
            c84move: [[0; PHASE2_MOVECOUNT]; C8_4D2],
        }
    }
}

struct Coord168 {
    pack168hi: [i32; 256],
    pack168lo: [i32; 256],
    c168move: [[usize; PHASE2_MOVECOUNT]; C16_8],
}

impl Coord for Coord168 {
    fn coordinate_for_pattern(&self, pattern: &PackedKPattern) -> usize {
        let mut bits = 0;
        // TODO: store this in the struct?
        let centers_orbit_info = &pattern
            .packed_orbit_data
            .packed_kpuzzle
            .data
            .orbit_iteration_info[2];
        assert!(centers_orbit_info.name == "CENTERS".into());
        for idx in [0, 1, 2, 3, 8, 9, 10, 11, 16, 17, 18, 19, 20, 21, 22, 23] {
            bits *= 2;
            if pattern.get_piece_or_permutation(centers_orbit_info, idx) == 0 {
                bits += 1
            }
        }
        (self.pack168hi[bits >> 8] + self.pack168lo[bits & 255]) as usize
    }

    fn main_table(&mut self) -> &mut [[usize; PHASE2_MOVECOUNT]] {
        &mut self.c168move
    }
}

impl Default for Coord168 {
    fn default() -> Self {
        Self {
            pack168hi: [0; 256],
            pack168lo: [0; 256],
            c168move: [[0; PHASE2_MOVECOUNT]; C16_8],
        }
    }
}

struct CoordEP {
    epmove: [[usize; PHASE2_MOVECOUNT]; EDGE_PARITY],
}

impl Coord for CoordEP {
    fn coordinate_for_pattern(&self, pattern: &PackedKPattern) -> usize {
        let mut bits = 0;
        let mut r = 0;
        // TODO: store this in the struct?
        let edges_orbit_info = &pattern
            .packed_orbit_data
            .packed_kpuzzle
            .data
            .orbit_iteration_info[1];
        assert!(edges_orbit_info.name == "WINGS".into());
        for idx in 0..24 {
            if ((bits >> idx) & 1) == 0 {
                let mut cyclen = 0;
                let mut j: usize = idx;
                while ((bits >> j) & 1) == 0 {
                    cyclen += 1;
                    bits |= 1 << j;
                    j = pattern.get_piece_or_permutation(edges_orbit_info, j) as usize;
                }
                r += cyclen + 1;
            }
        }
        return (r & 1) as usize;
    }

    fn main_table(&mut self) -> &mut [[usize; PHASE2_MOVECOUNT]] {
        &mut self.epmove
    }
}

impl Default for CoordEP {
    fn default() -> Self {
        Self {
            epmove: [[0; PHASE2_MOVECOUNT]; EDGE_PARITY],
        }
    }
}

struct Phase2SymmCoords {
    packed_kpuzzle: PackedKPuzzle,
    phase2prune: [u8; PHASE2PRUNE_SIZE],
    coord_84: Coord84,
    coord_168: Coord168,
    coord_ep: CoordEP,
}

impl Phase2SymmCoords {
    fn bitcount(mut bits: usize) -> i32 {
        let mut r = 0;
        while bits != 0 {
            r += 1;
            bits &= bits - 1;
        }
        r
    }
    fn init_choose_tables(&mut self) {
        let mut at = 0;
        for i in 0..128 {
            if Phase2SymmCoords::bitcount(i) == 4 {
                self.coord_84.pack84[i] = at;
                self.coord_84.pack84[255 - i] = at;
                at += 1;
            }
        }
        for i in 0..256 {
            self.coord_168.pack168hi[i] = -1;
            self.coord_168.pack168lo[i] = -1;
        }
        at = 0;
        for i in 0..0x10000 {
            if Phase2SymmCoords::bitcount(i) == 8 {
                if self.coord_168.pack168hi[i >> 8] < 0 {
                    self.coord_168.pack168hi[i >> 8] = at;
                }
                if self.coord_168.pack168lo[i & 255] < 0 {
                    self.coord_168.pack168lo[i & 255] = at - self.coord_168.pack168hi[i >> 8];
                }
                at += 1;
            }
        }
    }
    fn fillmovetable(&mut self, coordinate_table: CoordinateTable, moves: &SearchGenerators) {
        // TODO: double-check if there are any performance penalties for `dyn`.
        let coord_field: &mut dyn Coord = match coordinate_table {
            CoordinateTable::Coord84 => &mut self.coord_84,
            CoordinateTable::Coord168 => &mut self.coord_168,
            CoordinateTable::Coordep => &mut self.coord_ep,
        };
        {
            let tab = coord_field.main_table();
            for i in 0..tab.len() {
                tab[i][0] = INF;
            }
        }
        let mut q: Vec<PackedKPattern> = Vec::new();
        q.push(match coordinate_table {
            CoordinateTable::Coordep => self.packed_kpuzzle.default_pattern(),
            _ => cube4x4x4_phase2_target_pattern().clone()
        });
        let mut qget = 0;
        let mut qput = 1;
        while qget < qput {
            let src = coord_field.coordinate_for_pattern(&q[qget]);
            coord_field.main_table()[src][0] = 0;
            let mut moveind = 0;
            for m in &moves.flat {
                let dststate = q[qget].clone().apply_transformation(&m.transformation);
                let dst = coord_field.coordinate_for_pattern(&dststate);
                let tab = coord_field.main_table();
                tab[src][moveind] = dst;
                if tab[dst][0] == INF {
                    tab[dst][0] = 0;
                    q.push(dststate.clone());
                    qput += 1;
                }
                tab[src][moveind] = dst;
                moveind += 1;
            }
            qget += 1;
        }

        let tab = coord_field.main_table();
        assert!(qget == tab.len());
        assert!(qput == tab.len());
    }
    fn init_move_tables(&mut self) {
        self.packed_kpuzzle = cube4x4x4_packed_kpuzzle();
        // TODO: deduplicate against earlier constant above
        let phase2_generators =
            generators_from_vec_str(vec!["Uw2", "U", "L", "F", "Rw", "R", "B", "Dw2", "D"]);
        match SearchGenerators::try_new(
            &self.packed_kpuzzle,
            &phase2_generators,
            &MetricEnum::Hand,
            false,
        ) {
            Result::Ok(moves) => {
                self.fillmovetable(CoordinateTable::Coord84, &moves);
                self.fillmovetable(CoordinateTable::Coord168, &moves);
                self.fillmovetable(CoordinateTable::Coordep, &moves);
            }
            _ => {
                panic!();
            }
        }
    }
    fn new(puz: PackedKPuzzle) -> Self {
        Self {
            packed_kpuzzle: puz,
            phase2prune: [255; PHASE2PRUNE_SIZE],
            coord_84: Coord84::default(),
            coord_168: Coord168::default(),
            coord_ep: CoordEP::default(),
        }
    }
}

struct Phase2AdditionalSolutionCondition {
    packed_kpuzzle: PackedKPuzzle, // we could theoretically get this from `main_search_pattern`, but this way is more clear.
    phase2_search_full_pattern: PackedKPattern,
    _debug_num_checked: usize,                         // TODO: remove
    _debug_num_centers_rejected: usize,                // TODO: remove
    _debug_num_total_rejected: usize,                  // TODO: remove
    _debug_num_basic_parity_rejected: usize,           // TODO: remove
    _debug_num_known_pair_orientation_rejected: usize, // TODO: remove
    _debug_num_edge_parity_rejected: usize,            // TODO: remove
}

impl Phase2AdditionalSolutionCondition {
    fn log(&self) {
        if !self._debug_num_total_rejected.is_power_of_two() {
            return;
        }
        println!(
            "{} total phase 2 rejections ({} centers, {} basic parity, {} known pair orientation, {} edge parity)",
            self._debug_num_total_rejected,
            self._debug_num_centers_rejected,
            self._debug_num_basic_parity_rejected,
            self._debug_num_known_pair_orientation_rejected,
            self._debug_num_edge_parity_rejected,
        );
    }

    // fn debug_record_centers_rejection(&mut self) {
    //     self._debug_num_total_rejected += 1;
    //     self._debug_num_centers_rejected += 1;
    //     self.log()
    // }

    // fn debug_record_basic_parity_rejection(&mut self) {
    //     self._debug_num_total_rejected += 1;
    //     self._debug_num_basic_parity_rejected += 1;
    //     self.log()
    // }

    // fn debug_record_known_pair_orientation_rejection(&mut self) {
    //     self._debug_num_total_rejected += 1;
    //     self._debug_num_known_pair_orientation_rejected += 1;
    //     self.log()
    // }

    // fn debug_record_edge_parity_rejection(&mut self) {
    //     self._debug_num_total_rejected += 1;
    //     self._debug_num_edge_parity_rejected += 1;
    //     self.log()
    // }
}

// TODO: change the 4x4x4 Speffz def to have indistinguishable centers and get rid of this.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SideCenter {
    L,
    R,
}

const PHASE2_SOLVED_SIDE_CENTER_CASES: [[[SideCenter; 4]; 2]; 12] = [
    // flat faces
    [
        [SideCenter::L, SideCenter::L, SideCenter::L, SideCenter::L],
        [SideCenter::R, SideCenter::R, SideCenter::R, SideCenter::R],
    ],
    [
        [SideCenter::R, SideCenter::R, SideCenter::R, SideCenter::R],
        [SideCenter::L, SideCenter::L, SideCenter::L, SideCenter::L],
    ],
    // horizontal bars
    [
        [SideCenter::L, SideCenter::L, SideCenter::R, SideCenter::R],
        [SideCenter::L, SideCenter::L, SideCenter::R, SideCenter::R],
    ],
    [
        [SideCenter::R, SideCenter::R, SideCenter::L, SideCenter::L],
        [SideCenter::R, SideCenter::R, SideCenter::L, SideCenter::L],
    ],
    [
        [SideCenter::R, SideCenter::R, SideCenter::L, SideCenter::L],
        [SideCenter::L, SideCenter::L, SideCenter::R, SideCenter::R],
    ],
    [
        [SideCenter::L, SideCenter::L, SideCenter::R, SideCenter::R],
        [SideCenter::R, SideCenter::R, SideCenter::L, SideCenter::L],
    ],
    // vertical bars
    [
        [SideCenter::L, SideCenter::R, SideCenter::R, SideCenter::L],
        [SideCenter::L, SideCenter::R, SideCenter::R, SideCenter::L],
    ],
    [
        [SideCenter::R, SideCenter::L, SideCenter::L, SideCenter::R],
        [SideCenter::R, SideCenter::L, SideCenter::L, SideCenter::R],
    ],
    [
        [SideCenter::L, SideCenter::R, SideCenter::R, SideCenter::L],
        [SideCenter::R, SideCenter::L, SideCenter::L, SideCenter::R],
    ],
    [
        [SideCenter::R, SideCenter::L, SideCenter::L, SideCenter::R],
        [SideCenter::L, SideCenter::R, SideCenter::R, SideCenter::L],
    ],
    // checkerboards
    [
        [SideCenter::L, SideCenter::R, SideCenter::L, SideCenter::R],
        [SideCenter::L, SideCenter::R, SideCenter::L, SideCenter::R],
    ],
    [
        [SideCenter::R, SideCenter::L, SideCenter::R, SideCenter::L],
        [SideCenter::R, SideCenter::L, SideCenter::R, SideCenter::L],
    ],
];

fn is_solve_center_center_case(case: &[[SideCenter; 4]; 2]) -> bool {
    for phase2_solved_side_center_case in PHASE2_SOLVED_SIDE_CENTER_CASES {
        if &phase2_solved_side_center_case == case {
            return true;
        }
    }
    false
}

impl AdditionalSolutionCondition for Phase2AdditionalSolutionCondition {
    fn should_accept_solution(
        &mut self,
        _candidate_pattern: &PackedKPattern,
        candidate_alg: &Alg,
    ) -> bool {
        let mut accept = true;

        // self._debug_num_checked += 1;
        // if self._debug_num_checked.is_power_of_two() {
        //     println!(
        //         "Alg ({} checked): {}",
        //         self._debug_num_checked, candidate_alg
        //     )
        // }

        // dbg!(&candidate_alg.to_string());
        let transformation = self
            .packed_kpuzzle
            .transformation_from_alg(candidate_alg)
            .expect("Internal error applying an alg from a search result.");
        let pattern_with_alg_applied = self
            .phase2_search_full_pattern
            .apply_transformation(&transformation);

        /******** Centers ********/

        // TODO: is it more efficient to check this later?

        let centers_orbit_info = &self.packed_kpuzzle.data.orbit_iteration_info[2];
        assert!(centers_orbit_info.name == "CENTERS".into());

        #[allow(non_snake_case)] // Speffz
        let [E, F, G, H, M, N, O, P] = [4, 5, 6, 7, 12, 13, 14, 15].map(|idx| {
            if pattern_with_alg_applied.get_piece_or_permutation(centers_orbit_info, idx) < 8 {
                SideCenter::L
            } else {
                SideCenter::R
            }
        });
        if !is_solve_center_center_case(&[[E, F, G, H], [M, N, O, P]]) {
            {
                self._debug_num_centers_rejected += 1;
            }
            accept = false;
        }

        /******** Edges ********/

        let wings_orbit_info = &self.packed_kpuzzle.data.orbit_iteration_info[1];
        assert!(wings_orbit_info.name == "WINGS".into());

        if basic_parity(
            &pattern_with_alg_applied.packed_orbit_data.byte_slice()[wings_orbit_info
                .pieces_or_pemutations_offset
                ..wings_orbit_info.orientations_offset],
        ) != BasicParity::Even
        {
            // println!("false1: {}", candidate_alg);
            {
                self._debug_num_basic_parity_rejected += 1;
            }
            accept = false;
        }

        let mut edge_parity = 0;
        // Indexed by the value stored in an `EdgePairIndex` (i.e. half of the entries will always be `Unknown`).
        let mut known_pair_orientations = vec![Phase2EdgeOrientation::Unknown; NUM_4X4X4_EDGES];
        let mut known_pair_inc = 1;
        for position in 0..23 {
            // dbg!(position);
            let position_is_high = is_high(position);

            let piece = pattern_with_alg_applied
                .packed_orbit_data
                .get_packed_piece_or_permutation(wings_orbit_info, position);
            let piece_is_high = is_high(piece as usize);

            let pair_orientation = if piece_is_high == position_is_high {
                Phase2EdgeOrientation::Oriented
            } else {
                edge_parity += 1;
                Phase2EdgeOrientation::Misoriented
            };

            let edge_pair_index: EdgePairIndex = EDGE_TO_INDEX[piece as usize];
            // println!(
            //     "comparin': {}, {}, {}, {}, {}, {}, {:?}",
            //     candidate_alg,
            //     position,
            //     piece,
            //     piece_is_high,
            //     position_is_high,
            //     edge_pair_index.0,
            //     pair_orientation
            // );
            match &known_pair_orientations[edge_pair_index.0] {
                Phase2EdgeOrientation::Unknown => {
                    // println!(
                    //     "known_pair_orientations[{}] = {:?} ({}, {})",
                    //     edge_pair_index.0, pair_orientation, piece_is_high, position_is_high
                    // );
                    known_pair_orientations[edge_pair_index.0] = pair_orientation
                }
                known_pair_orientation => {
                    if known_pair_orientation != &pair_orientation {
                        // println!("false2 {:?}", known_pair_orientation);
                        {
                            self._debug_num_known_pair_orientation_rejected += known_pair_inc;
                            known_pair_inc = 0;
                        }
                        accept = false;
                    }
                }
            }
        }
        if edge_parity % 4 != 0 {
            // println!("false3: {}, {}", candidate_alg, edge_parity);
            {
                self._debug_num_edge_parity_rejected += 1;
            }
            accept = false;
        }

        if !accept {
            self._debug_num_total_rejected += 1;
            self.log()
        }

        // println!("true: {}", candidate_alg);
        accept
    }
}

impl Scramble4x4x4FourPhase {
    pub(crate) fn solve_4x4x4_pattern(
        &mut self,
        main_search_pattern: &PackedKPattern, // TODO: avoid assuming a superpattern.
    ) -> Alg {
        dbg!("solve_4x4x4_pattern");
        let mut x = Phase2SymmCoords::new(self.packed_kpuzzle.clone());
        x.init_choose_tables();
        x.init_move_tables();
        let phase1_alg = {
            let mut phase1_search_pattern = self.phase1_target_pattern.clone();
            for orbit_info in &self.packed_kpuzzle.data.orbit_iteration_info {
                for i in 0..orbit_info.num_pieces {
                    remap_piece_for_search_pattern(
                        orbit_info,
                        main_search_pattern,
                        &self.phase1_target_pattern,
                        &mut phase1_search_pattern,
                        i,
                    );
                    if orbit_info.name == "CORNERS".into() {
                        // TODO: handle this properly by taking into account orientation mod.
                        phase1_search_pattern
                            .packed_orbit_data
                            .set_packed_orientation(orbit_info, i, 3);
                    }
                    if orbit_info.name == "WINGS".into() {
                        // TODO: handle this properly by taking into account orientation mod.
                        phase1_search_pattern
                            .packed_orbit_data
                            .set_packed_orientation(orbit_info, i, 2);
                    }
                }
            }

            self.phase1_idfs
                .search(
                    &phase1_search_pattern,
                    IndividualSearchOptions {
                        min_num_solutions: Some(1),
                        min_depth: None,
                        max_depth: None,
                        disallowed_initial_quanta: None,
                        disallowed_final_quanta: None,
                    },
                )
                .next()
                .unwrap()
        };

        dbg!(&phase1_alg.to_string());

        let mut phase2_alg = {
            // TODO: unify with phase 1 (almost identical code)
            let mut phase2_search_pattern = self.phase2_center_target_pattern.clone();
            for orbit_info in &self.packed_kpuzzle.data.orbit_iteration_info {
                for i in 0..orbit_info.num_pieces {
                    remap_piece_for_search_pattern(
                        orbit_info,
                        main_search_pattern,
                        &self.phase2_center_target_pattern,
                        &mut phase2_search_pattern,
                        i,
                    );
                    if orbit_info.name == "CORNERS".into() {
                        // TODO: handle this properly by taking into account orientation mod.
                        phase2_search_pattern
                            .packed_orbit_data
                            .set_packed_orientation(orbit_info, i, 3);
                    }
                    if orbit_info.name == "WINGS".into() {
                        // TODO: handle this properly by taking into account orientation mod.
                        phase2_search_pattern
                            .packed_orbit_data
                            .set_packed_orientation(orbit_info, i, 2);
                    }
                }
            }
            let phase2_search_pattern = phase2_search_pattern.apply_transformation(
                &self
                    .packed_kpuzzle
                    .transformation_from_alg(&phase1_alg)
                    .unwrap(),
            );
            let phase2_search_full_pattern = main_search_pattern.apply_transformation(
                &self
                    .packed_kpuzzle
                    .transformation_from_alg(&phase1_alg)
                    .unwrap(),
            );

            let additional_solution_condition = Phase2AdditionalSolutionCondition {
                packed_kpuzzle: self.packed_kpuzzle.clone(),
                phase2_search_full_pattern,
                _debug_num_checked: 0,
                _debug_num_centers_rejected: 0,
                _debug_num_total_rejected: 0,
                _debug_num_basic_parity_rejected: 0,
                _debug_num_known_pair_orientation_rejected: 0,
                _debug_num_edge_parity_rejected: 0,
            };

            self.phase2_idfs
                .search_with_additional_check(
                    &phase2_search_pattern,
                    IndividualSearchOptions {
                        min_num_solutions: Some(1), // TODO
                        min_depth: None,
                        max_depth: None,
                        disallowed_initial_quanta: None,
                        disallowed_final_quanta: None,
                    },
                    Some(Box::new(additional_solution_condition)),
                )
                .next()
                .unwrap()
            // dbg!(&phase2_search_pattern);
            // dbg!(&self.phase2_center_target_pattern);
            // dbg!(phase2_search_pattern == self.phase2_center_target_pattern);
            // 'search_loop: loop {}
        };

        let mut nodes = phase1_alg.nodes;
        nodes.push(cubing::alg::AlgNode::PauseNode(Pause {}));
        nodes.append(&mut phase2_alg.nodes);
        nodes.push(cubing::alg::AlgNode::PauseNode(Pause {}));
        Alg { nodes }
    }

    // TODO: rely on the main search to find patterns at a low depth?
    pub fn is_valid_scramble_pattern(&mut self, _pattern: &PackedKPattern) -> bool {
        eprintln!("WARNING: FILTERING DISABLED FOR TESTING"); // TODO
        true
        // self.filtering_idfs
        //     .search(
        //         pattern,
        //         IndividualSearchOptions {
        //             min_num_solutions: Some(1),
        //             min_depth: Some(0),
        //             max_depth: Some(2),
        //             disallowed_initial_quanta: None,
        //             disallowed_final_quanta: None,
        //         },
        //     )
        //     .next()
        //     .is_none()
    }

    pub(crate) fn scramble_4x4x4(&mut self) -> Alg {
        loop {
            let hardcoded_scramble_alg_for_testing ="F' R' B2 D L' B D L2 F L2 F2 B' L2 U2 F2 U2 F' R2 L2 D' L2 Fw2 Rw2 R F' Uw2 U2 Fw2 F Uw2 L U2 R2 D2 Uw U F R F' Rw' Fw B Uw' L' Fw2 F2".parse::<Alg>().unwrap();
            // let hardcoded_scramble_alg_for_testing =
            //     "r U2 x r U2 r U2 r' U2 l U2 r' U2 r U2 r' U2 r'"
            //         .parse::<Alg>()
            //         .unwrap();
            // let hardcoded_scramble_alg_for_testing =
            //     "Uw2 Fw2 U' L2 F2 L' Uw2 Fw2 U D' L' U2 R' Fw D' Rw2 F' L2 Uw' //Fw L U' R2 Uw Fw"
            //         .parse::<Alg>()
            //         .unwrap();
            let scramble_pattern = random_4x4x4_pattern(Some(&hardcoded_scramble_alg_for_testing));

            if !self.is_valid_scramble_pattern(&scramble_pattern) {
                continue;
            }
            dbg!(hardcoded_scramble_alg_for_testing.to_string());
            let solution_alg = self.solve_4x4x4_pattern(&scramble_pattern);
            println!(
                "{}",
                twizzle_link(&hardcoded_scramble_alg_for_testing, &solution_alg)
            );
            return solution_alg;
        }
    }
}

fn remap_piece_for_search_pattern(
    orbit_info: &PackedKPuzzleOrbitInfo,
    from_pattern: &PackedKPattern,
    target_pattern: &PackedKPattern,
    search_pattern: &mut PackedKPattern,
    i: usize,
) {
    let old_piece = from_pattern
        .packed_orbit_data
        .get_packed_piece_or_permutation(orbit_info, i);
    let old_piece_mapped = target_pattern
        .packed_orbit_data
        .get_packed_piece_or_permutation(orbit_info, old_piece as usize);
    search_pattern
        .packed_orbit_data
        .set_packed_piece_or_permutation(orbit_info, i, old_piece_mapped);
    let ori = from_pattern
        .packed_orbit_data
        .get_packed_orientation(orbit_info, i);
    search_pattern
        .packed_orbit_data
        .set_packed_orientation(orbit_info, i, ori);
    if orbit_info.name == "CORNERS".into() {
        // TODO: handle this properly by taking into account orientation mod.
        search_pattern
            .packed_orbit_data
            .set_packed_orientation(orbit_info, i, 3);
    }
}

// TODO: switch to `LazyLock` once that's stable: https://doc.rust-lang.org/nightly/std/cell/struct.LazyCell.html
lazy_static! {
    static ref SCRAMBLE4X4X4_FOUR_PHASE: Mutex<Scramble4x4x4FourPhase> =
        Mutex::new(Scramble4x4x4FourPhase::default());
}

pub fn scramble_4x4x4() -> Alg {
    SCRAMBLE4X4X4_FOUR_PHASE.lock().unwrap().scramble_4x4x4()
}

// TODO: remove `url` crate when removing this.
pub fn twizzle_link(scramble: &Alg, solution: &Alg) -> String {
    let mut url = Url::parse("https://alpha.twizzle.net/edit/").unwrap();
    url.query_pairs_mut()
        .append_pair("setup-alg", &scramble.to_string());
    url.query_pairs_mut()
        .append_pair("alg", &solution.to_string());
    url.query_pairs_mut().append_pair("puzzle", "4x4x4");
    url.to_string()
}