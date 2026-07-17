use crate::domain::Distance;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ScoreResult {
    NoCapacityViolation(MediumSoft),
    CapacityViolation,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct MediumSoft {
    // maximize
    pub(crate) medium_score: usize,
    // minimize
    pub(crate) soft_penalty: Distance,
}
impl MediumSoft {
    pub(crate) const WORST: Self = Self {
        medium_score: 0,
        soft_penalty: Distance(f64::MAX),
    };

    pub(crate) const fn new(medium_score: usize, soft_penalty: Distance) -> Self {
        Self {
            medium_score,
            soft_penalty,
        }
    }

    /// Calculates the 'delta' between two scores.
    ///
    /// If the medium score differs, it will return the absolute difference
    /// between the scores `>= 1.0`. If the medium score is equal it will return
    /// a percentage of difference of the soft penalty in `0.0-1.0` range.
    pub(crate) fn delta(self, other: Self) -> f64 {
        if self.medium_score != other.medium_score {
            self.medium_score.abs_diff(other.medium_score) as f64
        } else if other.soft_penalty < self.soft_penalty {
            1.0 - other.soft_penalty.0 / self.soft_penalty.0
        } else {
            1.0 - self.soft_penalty.0 / other.soft_penalty.0
        }
    }
}

impl PartialOrd for MediumSoft {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.medium_score != other.medium_score {
            return Some(self.medium_score.cmp(&other.medium_score));
        }
        Some(
            self.soft_penalty
                .partial_cmp(&other.soft_penalty)?
                .reverse(),
        )
    }
}

impl std::fmt::Debug for MediumSoft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MediumSoft({}, -{})",
            self.medium_score, self.soft_penalty.0
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn score_ordering_maximizes_medium_score_and_minimizes_soft_penalty() {
        assert!(MediumSoft::new(0, Distance(0.0)) > MediumSoft::new(0, Distance(1.0)));
        assert!(MediumSoft::new(1, Distance(1.0)) > MediumSoft::new(0, Distance(0.0)));

        let mut scores = vec![
            MediumSoft::new(20, Distance(70.0)),
            MediumSoft::new(100, Distance(1000.0)),
            MediumSoft::new(50, Distance(1.0)),
            MediumSoft::new(100, Distance(500.0)),
            MediumSoft::new(20, Distance(0.0)),
        ];
        scores.sort_by(|a, b| a.partial_cmp(b).unwrap());

        assert_eq!(
            scores,
            vec![
                MediumSoft::new(20, Distance(70.0)),
                MediumSoft::new(20, Distance(0.0)),
                MediumSoft::new(50, Distance(1.0)),
                MediumSoft::new(100, Distance(1000.0)),
                MediumSoft::new(100, Distance(500.0)),
            ],
        );
    }
}
