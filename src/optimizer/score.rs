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
    pub(crate) const ZERO: Self = Self {
        medium_score: 0,
        soft_penalty: Distance(0.0),
    };

    pub(crate) const fn new(medium_score: usize, soft_penalty: Distance) -> Self {
        Self {
            medium_score,
            soft_penalty,
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
