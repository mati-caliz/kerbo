class Base {
  constructor() {
    this.ready = true;
  }
}

class Missing extends Base {
  constructor() {
    this.value = 1;
  }
}

class Some extends Base {
  constructor(flag) {
    if (flag) {
      super();
    }
  }
}

class Twice extends Base {
  constructor() {
    super();
    super();
  }
}

class Looped extends Base {
  constructor(items) {
    for (const item of items) {
      super(item);
    }
  }
}

class WhileLooped extends Base {
  constructor(count) {
    while (count--) {
      super();
    }
  }
}

class NullBase extends null {
  constructor() {
    super();
  }
}

class Literal extends 42 {
  constructor() {
    super();
  }
}

class Arrow extends (() => Base) {
  constructor() {
    super();
  }
}

class Possible extends (flag ? Base : Object) {
  constructor() {
    super();
  }
}

class Returned extends Base {
  constructor() {
    return {};
  }
}

class BeforeSuper extends Base {
  constructor() {
    this.early = 1;
    super.method();
    super();
    this.late = 2;
  }
}

class BranchBefore extends Base {
  constructor(flag) {
    if (flag) {
      super();
    }
    this.value = flag;
  }
}

class Nested extends Base {
  constructor() {
    const read = () => this.value;
    super();
    read();
  }
}

class Thrown extends Base {
  constructor() {
    throw new Error("nunca");
  }
}

class Switched extends Base {
  constructor(kind) {
    switch (kind) {
      case 1:
        super(1);
        break;
      default:
        super(2);
    }
  }
}

class ForUpdate extends Base {
  constructor() {
    for (let index = 0; index < 1; super()) {
      this.index = index;
    }
  }
}

class Argument extends Base {
  constructor() {
    super(this.value);
  }
}

class Static extends Base {
  static constructor() {
    return 1;
  }
}

class Quoted extends Base {
  "constructor"() {}
}

const Expression = class extends Base {
  constructor() {}
};

export { Missing, Some, Twice, Looped, WhileLooped, NullBase, Literal, Arrow, Possible, Returned };
export { BeforeSuper, BranchBefore, Nested, Thrown, Switched, ForUpdate, Argument, Static, Quoted, Expression };
