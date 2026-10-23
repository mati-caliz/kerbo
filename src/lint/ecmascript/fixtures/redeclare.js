var total = 0;
var total = 1;

function load() {}
var load;

function save(name) {
  var name = name.trim();
  return name;
}

function loops() {
  for (var index = 0; index < 2; index++) {}
  for (var index = 0; index < 2; index++) {}
}

let outer = 1;
{
  let outer = 2;
  console.log(outer);
}

switch (total) {
  case 0: {
    const shown = 1;
    console.log(shown);
  }
}

try {
  load();
} catch (error) {
  var error = 1;
}

function render() {}
render = null;
render += 1;
[render] = [1];
({ render } = {});

const named = function self() {
  self = 1;
};

function first() {}
function first() {}

export { total, save, loops, outer, error, named, first };
