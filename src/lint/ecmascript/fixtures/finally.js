function returns() {
  try {
    work();
  } finally {
    return 1;
  }
}

function throws() {
  try {
    work();
  } catch (error) {
    report(error);
  } finally {
    throw new Error("cerrado");
  }
}

function nestedFunctions() {
  try {
    work();
  } finally {
    const later = () => {
      return 2;
    };
    function named() {
      return 3;
    }
    class Holder {
      method() {
        return 4;
      }
    }
    later(named, Holder);
  }
}

function loops() {
  for (const item of items) {
    try {
      work(item);
    } finally {
      if (item) {
        break;
      }
      continue;
    }
  }
  while (ready()) {
    try {
      work();
    } finally {
      for (;;) {
        break;
      }
      do {
        continue;
      } while (ready());
      switch (state()) {
        case 1:
          break;
        default:
          continue;
      }
    }
  }
}

function labels() {
  outer: for (const item of items) {
    try {
      work(item);
    } finally {
      inner: for (const other of item) {
        break outer;
      }
      block: {
        break block;
      }
      for (const other of item) {
        continue inner;
      }
      continue outer;
    }
  }
}

function nestedTry() {
  try {
    work();
  } finally {
    try {
      return 5;
    } finally {
      cleanup();
    }
  }
}
