use std::{process::exit, time::Instant};

use cubing::alg::Move;

use crate::{
    CanonicalFSM, CanonicalFSMState, MoveClassIndex, PackedKPattern, PackedKPuzzle, SearchError,
    SearchMoveCache, CANONICAL_FSM_START_STATE,
};

pub struct IDFSearch {
    pub search_move_cache: SearchMoveCache,
    pub canonical_fsm: CanonicalFSM,
    pub packed_kpuzzle: PackedKPuzzle,
    pub target_pattern: PackedKPattern,
    pub scramble_pattern: PackedKPattern,
}

impl IDFSearch {
    pub fn try_new(
        packed_kpuzzle: PackedKPuzzle,
        target_pattern: PackedKPattern,
        move_list: Vec<Move>,
        scramble_pattern: PackedKPattern,
    ) -> Result<Self, SearchError> {
        let search_move_cache = SearchMoveCache::try_new(&packed_kpuzzle, &move_list)?;
        let canonical_fsm = CanonicalFSM::try_new(search_move_cache.clone())?; // TODO: avoid a clone
        Ok(Self {
            search_move_cache,
            canonical_fsm,
            packed_kpuzzle,
            target_pattern,
            scramble_pattern,
        })
    }

    pub fn search(&self) -> Result<(), SearchError> {
        let start_time = Instant::now();
        let mut remaining_depth = 0;
        loop {
            println!("Searching to depth: {}", remaining_depth);
            if self.recurse(
                &self.target_pattern,
                CANONICAL_FSM_START_STATE,
                remaining_depth,
            ) {
                println!("Found a solution at depth: {}", remaining_depth);
                println!("Found in: {:?}", Instant::now() - start_time);
                exit(0);
            }

            remaining_depth += 1;
        }
    }

    fn recurse(
        &self,
        current_pattern: &PackedKPattern,
        current_state: CanonicalFSMState,
        remaining_depth: usize,
    ) -> bool {
        if remaining_depth == 0 {
            return current_pattern == &self.scramble_pattern;
        }
        for (move_class_index, move_transformation_multiples) in
            self.search_move_cache.grouped.iter().enumerate()
        {
            let next_state = match self
                .canonical_fsm
                .next_state(current_state, MoveClassIndex(move_class_index))
            {
                Some(next_state) => next_state,
                None => {
                    continue;
                }
            };

            for move_transformation_info in move_transformation_multiples {
                if self.recurse(
                    &current_pattern.apply_transformation(&move_transformation_info.transformation),
                    next_state,
                    remaining_depth - 1,
                ) {
                    return true;
                }
            }
        }
        false
    }
}
