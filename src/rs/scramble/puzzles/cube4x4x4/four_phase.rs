use std::{marker::PhantomData, sync::Arc};

use cubing::{
    alg::{parse_alg, Alg, Pause},
    kpuzzle::{KPattern, KPuzzle},
};

use url::Url;

use crate::{
    _internal::{
        options::{MetricEnum, VerbosityLevel},
        CanonicalFSM, IDFSearch, IndividualSearchOptions, SearchGenerators, SearchLogger,
    },
    scramble::{
        puzzles::{
            cube4x4x4::{
                phase2::{
                    pattern_to_phase2_pattern, remap_piece_for_phase1_or_phase2_search_pattern,
                },
                phase2_symmetry::Phase2SymmetryTables,
                random::random_4x4x4_pattern,
            },
            definitions::{cube4x4x4_kpuzzle, cube4x4x4_phase1_target_kpattern},
        },
        scramble_search::{basic_idfs, idfs_with_target_pattern},
    },
};

use super::{
    super::super::scramble_search::generators_from_vec_str,
    phase2_symmetry::{Phase2Puzzle, PHASE2_SOLVED_STATE},
};

pub(crate) struct Scramble4x4x4FourPhase {
    kpuzzle: KPuzzle,

    _filtering_idfs: IDFSearch<KPuzzle>,

    phase1_target_pattern: KPattern,
    phase1_idfs: IDFSearch<KPuzzle>,

    phase2_idfs: IDFSearch<Phase2Puzzle>,
}

impl Default for Scramble4x4x4FourPhase {
    fn default() -> Self {
        let kpuzzle = cube4x4x4_kpuzzle().clone();

        let phase1_generators = generators_from_vec_str(vec![
            "Uw", "U", "Lw", "L", "Fw", "F", "Rw", "R", "Bw", "B", "Dw", "D",
        ]);
        // TODO: support normalizing orientation/ignoring orientation/24 targets, so that this checks for unoriented distance to solved.
        let filtering_idfs = basic_idfs(&kpuzzle, phase1_generators.clone(), Some(32));

        let phase1_target_pattern = cube4x4x4_phase1_target_kpattern().clone();
        // dbg!(&phase1_target_pattern);
        let phase1_idfs = idfs_with_target_pattern(
            &kpuzzle,
            phase1_generators.clone(),
            phase1_target_pattern.clone(),
            None,
        );

        let phase2_idfs = {
            let phase2_symmetry_tables = Phase2SymmetryTables::new();

            let phase2_generators =
                generators_from_vec_str(vec!["Uw2", "U", "L", "F", "Rw", "R", "B", "Dw2", "D"]);
            let phase2_fsm_search_generators = SearchGenerators::<KPuzzle>::try_new(
                &kpuzzle,
                &phase2_generators,
                &MetricEnum::Hand,
                false,
            )
            .unwrap();
            let canonical_fsm =
                CanonicalFSM::<KPuzzle>::try_new(phase2_fsm_search_generators).unwrap();
            // Transfer the `canonical_fsm` fields into an instance with a different type.
            // This is not safe in general.
            let canonical_fsm = CanonicalFSM::<Phase2Puzzle> {
                next_state_lookup: canonical_fsm.next_state_lookup,
                move_class_indices: canonical_fsm.move_class_indices,
                _marker: PhantomData,
            };
            // dbg!(&phase2_center_target_pattern);
            let search_generators = phase2_symmetry_tables
                .phase2_puzzle
                .search_generators
                .clone();
            IDFSearch::try_new_core(
                phase2_symmetry_tables.phase2_puzzle,
                PHASE2_SOLVED_STATE, //: TPuzzle, //:, //:Pattern,
                Arc::new(SearchLogger {
                    verbosity: VerbosityLevel::Info,
                }), //: Arc<SearchLogger>,
                None,                //: Option<usize>,
                search_generators,   //: SearchGenerators<TPuzzle>,
                canonical_fsm,       //: CanonicalFSM<TPuzzle>,
            )
            .unwrap()
        };
        Self {
            kpuzzle,
            _filtering_idfs: filtering_idfs,
            phase1_target_pattern,
            phase1_idfs,
            phase2_idfs,
        }
    }
}

impl Scramble4x4x4FourPhase {
    pub(crate) fn solve_4x4x4_pattern(
        &mut self,
        main_search_pattern: &KPattern, // TODO: avoid assuming a superpattern.
    ) -> Alg {
        let phase1_alg = {
            let mut phase1_search_pattern = self.phase1_target_pattern.clone();
            for orbit_info in self.kpuzzle.orbit_info_iter() {
                for i in 0..orbit_info.num_pieces {
                    remap_piece_for_phase1_or_phase2_search_pattern(
                        orbit_info,
                        main_search_pattern,
                        &self.phase1_target_pattern,
                        &mut phase1_search_pattern,
                        i,
                    );
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
            let phase2_search_pattern = pattern_to_phase2_pattern(main_search_pattern);
            let phase2_search_pattern = phase2_search_pattern.apply_alg(&phase1_alg).unwrap();

            // let phase2_search_full_pattern = main_search_pattern.apply_alg(&phase1_alg).unwrap(); // TODO

            let phase2_search_pattern = self
                .phase2_idfs
                .api_data
                .tpuzzle
                .coordinate_for_pattern(&phase2_search_pattern);

            self.phase2_idfs
                .search_with_additional_check(
                    &phase2_search_pattern,
                    IndividualSearchOptions::default(),
                    None,
                )
                .next()
                .unwrap()
        };

        let mut nodes = phase1_alg.nodes;
        nodes.push(cubing::alg::AlgNode::PauseNode(Pause {}));
        nodes.append(&mut phase2_alg.nodes);
        nodes.push(cubing::alg::AlgNode::PauseNode(Pause {}));
        Alg { nodes }
    }

    // TODO: rely on the main search to find patterns at a low depth?
    pub fn is_valid_scramble_pattern(&mut self, _pattern: &KPattern) -> bool {
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
            let hardcoded_scramble_alg_for_testing = parse_alg!("F' R' B2 D L' B D L2 F L2 F2 B' L2 U2 F2 U2 F' R2 L2 D' L2 Fw2 Rw2 R F' Uw2 U2 Fw2 F Uw2 L U2 R2 D2 Uw U F R F' Rw' Fw B Uw' L' Fw2 F2");
            // let hardcoded_scramble_alg_for_testing = parse_alg!("2R u");
            // let hardcoded_scramble_alg_for_testing =
            //     parse_alg!("r U2 x r U2 r U2 r' U2 l U2 r' U2 r U2 r' U2 r'");
            // let hardcoded_scramble_alg_for_testing = parse_alg!(
            //     "Uw2 Fw2 U' L2 F2 L' Uw2 Fw2 U D' L' U2 R' Fw D' Rw2 F' L2 Uw' //Fw L U' R2 Uw Fw"
            // );
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