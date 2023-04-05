use std::marker::PhantomData;

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::*,
    plonk::*,
    poly::Rotation,
};

#[derive(Clone, Debug)]
struct FibonacciConfig {
    col_a: Column<Advice>,
    col_b: Column<Advice>,
    col_c: Column<Advice>,
    instance: Column<Instance>,
    selector: Selector,
}

#[derive(Clone, Debug)]
struct FibonacciChip<F: FieldExt> {
    config: FibonacciConfig,
    _marker: PhantomData<F>,
}

impl<F: FieldExt> FibonacciChip<F> {
    fn construct(config: FibonacciConfig) -> Self {
        Self { 
            config, 
            _marker: PhantomData,
        }
    }
    
    fn configure(meta: &mut ConstraintSystem<F>) -> FibonacciConfig {
        let col_a = meta.advice_column();
        let col_b = meta.advice_column();
        let col_c = meta.advice_column();
        let instance = meta.instance_column();
        let selector = meta.selector();
        
        meta.enable_equality(col_a);
        meta.enable_equality(col_b);
        meta.enable_equality(col_c);
        meta.enable_equality(instance);

        meta.create_gate("add", |meta| {
            let a = meta.query_advice(col_a, Rotation::cur());
            let b = meta.query_advice(col_b, Rotation::cur());
            let c = meta.query_advice(col_c, Rotation::cur());
            let s = meta.query_selector(selector);

            vec![s * (a + b - c)]
        });

        FibonacciConfig { 
            col_a, 
            col_b, 
            col_c,
            instance, 
            selector,
        }
    }

    fn assign_first_row(
        &self,
        mut layouter: impl Layouter<F>
    ) -> Result<(AssignedCell<F, F>, AssignedCell<F, F>, AssignedCell<F, F>), Error> {
        layouter.assign_region(
            || "first row", 
            |mut region| {
                self.config.selector.enable(&mut region, 0)?;

                let a_cell = region.assign_advice_from_instance(
                    || "f(0)", 
                    self.config.instance, 
                    0, 
                    self.config.col_a, 
                    0,
                )?;

                let b_cell = region.assign_advice_from_instance(
                    || "f(1)", 
                    self.config.instance, 
                    1, 
                    self.config.col_b, 
                    0,
                )?;

                let c_cell = region.assign_advice(
                    || "f(0) + f(1)", 
                    self.config.col_c, 
                    0, 
                    || a_cell.value().copied() + b_cell.value(),
                )?;

                Ok((a_cell, b_cell, c_cell))
            })
    }

    fn assign_row(
        &self,
        mut layouter: impl Layouter<F>,
        prev_b: &AssignedCell<F, F>,
        prev_c: &AssignedCell<F, F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "next row", 
            |mut region| {
                self.config.selector.enable(&mut region, 0)?;

                prev_b.copy_advice(
                    || "a", 
                    &mut region, 
                    self.config.col_a, 
                    0,
                )?;

                prev_c.copy_advice(
                    || "b", 
                    &mut region, 
                    self.config.col_b, 
                    0,
                )?;

                let c_cell = region.assign_advice(
                    || "c", 
                    self.config.col_c, 
                    0, 
                    || prev_b.value().copied() + prev_c.value(),
                )?;

                Ok(c_cell)

            })
    }

    fn expose_public(
        &self,
        mut layouter: impl Layouter<F>,
        cell: AssignedCell<F, F>,
        row: usize
    ) -> Result<(), Error> {
        layouter.constrain_instance(cell.cell(), self.config.instance, row)
    }
}

#[derive(Default)]
struct MyCircuit<F>(PhantomData<F>);

impl<F: FieldExt> Circuit<F> for MyCircuit<F> {
    type Config = FibonacciConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        FibonacciChip::configure(meta)
    }

    fn synthesize(&self, config: Self::Config, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        let chip = FibonacciChip::construct(config);

        let (_, mut prev_b, mut prev_c) = 
            chip.assign_first_row(layouter.namespace(|| "first row"))?;

        for _i in 3..10 {
            let c_cell = chip.assign_row(layouter.namespace(|| "next row"), &prev_b, &prev_c)?;
            prev_b = prev_c;
            prev_c = c_cell;
        }

        chip.expose_public(layouter.namespace(|| "out"), prev_c, 2)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;
    use super::MyCircuit;
    use halo2_proofs::{
        dev::MockProver,
        pasta::Fp,
    };

    #[test]
    fn fibonacci_example1() {
        let k = 4;

        let a = Fp::from(1);
        let b = Fp::from(1);
        let out = Fp::from(55);

        let circuit = MyCircuit(PhantomData);
        let mut public_input = vec![a, b, out];

        let prover = MockProver::run(k, &circuit, vec![public_input.clone()]).unwrap();
        prover.assert_satisfied();

        public_input[2] += Fp::one();
        let prover = MockProver::run(k, &circuit, vec![public_input]).unwrap();
        assert!(prover.verify().is_err());
    }
}