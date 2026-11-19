package sample.inferred;

import app.Made;

public class Factory {
    public Made make() {
        return new Made();
    }

    public interface BaseStep {
        void run(Factory factory);
    }

    public interface Step extends BaseStep {
    }

    public void then(Step step) {
        step.run(this);
    }
}
