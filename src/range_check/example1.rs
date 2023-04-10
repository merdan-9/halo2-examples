use std::marker::PhantomData;

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Layouter, Value, floor_planner::V1},
    plonk::{Advice, Assigned, Column, ConstraintSystem, Constraints, Error, Expression, Selector, Circuit},
    poly::Rotation,
};

#[derive(Clone, Debug)]
struct RangeConstrained<F: FieldExt, const RANGE: usize>(AssignedCell<Assigned<F>, F>);

#[derive(Clone, Debug)]
struct RangeCheckConfig<F: FieldExt, const RANGE: usize> {
    value: Column<Advice>,
    q_range_check: Selector,
    _marker: PhantomData<F>,
}

impl<F: FieldExt, const RANGE: usize> RangeCheckConfig<F, RANGE> {
    fn configure(meta: &mut ConstraintSystem<F>, value: Column<Advice>) -> Self {
        let q_range_check = meta.selector();

        meta.create_gate("range check", |meta| {
            let q = meta.query_selector(q_range_check);
            let value = meta.query_advice(value, Rotation::cur());

            let range_check = |range: usize, value: Expression<F>| {
                (1..range).fold(value.clone(), |expr, i| {
                    expr * (Expression::Constant(F::from(i as u64)) - value.clone())
                })
            };

            Constraints::with_selector(q, [("range check", range_check(RANGE, value))])
        });

        Self {
            value,
            q_range_check,
            _marker: PhantomData,
        }
    }

    fn assign(
        &self,
        mut layouter: impl Layouter<F>,
        value: Value<Assigned<F>>,
    ) -> Result<RangeConstrained<F, RANGE>, Error> {
        layouter.assign_region(
            || "Assign region", 
            |mut region| {
            let offset = 0;

            self.q_range_check.enable(&mut region, offset)?;

            region
                .assign_advice(|| "value", self.value, offset, || value)
                .map(RangeConstrained)

            },
        )
    }
}

#[derive(Default)]
struct MyCircuit<F: FieldExt, const RANGE: usize> {
    value: Value<Assigned<F>>,
}

impl<F: FieldExt, const RANGE: usize> Circuit<F> for MyCircuit<F, RANGE> {
    type Config = RangeCheckConfig<F, RANGE>;
    type FloorPlanner = V1;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let value = meta.advice_column();
        RangeCheckConfig::configure(meta, value)
    }

    fn synthesize(&self, config: Self::Config, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        config.assign(layouter.namespace(|| "Assign region"), self.value)?;

        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::{
        dev::MockProver,
        pasta::Fp,
    };

    #[test]
    fn test_range_check_1() {
        let k = 4;
        const RANGE: usize = 8;

        for i in 0..RANGE {
            let circuit = MyCircuit::<Fp, RANGE> {
                value: Value::known(Fp::from(i as u64).into()),
            };

            let prover = MockProver::run(k, &circuit, vec![]).unwrap();
            prover.assert_satisfied();
        }
    }
}