mod table;
use std::vec;

use table::*;

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Layouter, Value, floor_planner::V1},
    plonk::{Advice, Assigned, Column, ConstraintSystem, Constraints, Error, Expression, Selector, Circuit},
    poly::Rotation,
};


#[derive(Clone, Debug)]
/// A range-constrained value in the circuit produced by the RangeCheckConfig.
struct RangeConstrained<F: FieldExt, const RANGE: usize>(AssignedCell<Assigned<F>, F>);

#[derive(Clone, Debug)]
struct RangeCheckConfig<F: FieldExt, const RANGE: usize, const LOOKUP_RANGE: usize> {
    q_range_check: Selector,
    q_lookup: Selector,
    value: Column<Advice>,
    table: RangeTableConfig<F, LOOKUP_RANGE>,
}

impl<F: FieldExt, const RANGE: usize, const LOOKUP_RANGE: usize>
    RangeCheckConfig<F, RANGE, LOOKUP_RANGE>      
{
    pub fn configure(meta: &mut ConstraintSystem<F>, value: Column<Advice>) -> Self {
        let q_range_check = meta.selector();
        let q_lookup = meta.complex_selector();
        let table = RangeTableConfig::configure(meta);

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

        meta.lookup(|meta| {
            let q_lookup = meta.query_selector(q_lookup);
            let value = meta.query_advice(value, Rotation::cur());

            vec![(q_lookup * value, table.value)]
        });

        Self {
            q_range_check,
            q_lookup,
            value,
            table,
        }
    }

    fn assign_simple(
        &self,
        mut layouter: impl Layouter<F>,
        value: Value<Assigned<F>>,
    ) -> Result<RangeConstrained<F, RANGE>, Error> {
        layouter.assign_region(
            || "Assign for simple",
            |mut region| {
                let offset = 0;
                self.q_range_check.enable(&mut region, offset)?;

                region
                    .assign_advice(|| "value", self.value, offset, || value)
                    .map(RangeConstrained)
            }, 
        )
    }

    fn assign_lookup(
        &self,
        mut layouter: impl Layouter<F>,
        value: Value<Assigned<F>>,
    ) -> Result<RangeConstrained<F, LOOKUP_RANGE>, Error> {
        layouter.assign_region(
            || "Assign for lookup",
            |mut region| {
                let offset = 0;
                self.q_lookup.enable(&mut region, offset)?;

                region
                    .assign_advice(|| "value", self.value, offset, || value)
                    .map(RangeConstrained)
            }, 
        )
    }
}

#[derive(Default)]
struct MyCircuit<F: FieldExt, const RANGE: usize, const LOOKUP_RANGE: usize> {
    value: Value<Assigned<F>>,
    lookup_value: Value<Assigned<F>>,
}

impl<F: FieldExt, const RANGE: usize, const LOOKUP_RANGE: usize> Circuit<F>
    for MyCircuit<F, RANGE, LOOKUP_RANGE>
{   
    type Config = RangeCheckConfig<F, RANGE, LOOKUP_RANGE>;
    type FloorPlanner = V1;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let value = meta.advice_column();
        RangeCheckConfig::configure(meta, value)
    }

    fn synthesize(&self, config: Self::Config, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        config.table.load(&mut layouter)?;

        config.assign_simple(layouter.namespace(|| "Assign for simple"), self.value)?;
        config.assign_lookup(layouter.namespace(|| "Assign for lookup"), self.lookup_value)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{
        dev::MockProver,
        pasta::Fp,
    };

    use super::*;

    #[test]
    fn test_range_check_2() {
        let k = 9;
        const RANGE: usize = 8;
        const LOOKUP_RANGE: usize = 256;

        for i in 0..RANGE {
            for j in 0..LOOKUP_RANGE {
                let circuit = MyCircuit::<Fp, RANGE, LOOKUP_RANGE> {
                    value: Value::known(Fp::from(i as u64).into()),
                    lookup_value: Value::known(Fp::from(j as u64).into()),
                };

                let prover = MockProver::run(k, &circuit, vec![]).unwrap();
                prover.assert_satisfied();
            }
        }
    }
}