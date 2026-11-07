# Pedido de cada sesión del piloto automático

Lo lee cada sesión que abre `scripts/autopilot/sesion.ps1`. El mensaje inicial dice la etapa y el
carril; esto dice cómo trabajar.

1. Leé la etapa en `ROADMAP.md` y tomá el próximo avance que se pueda cerrar entero: con sus tests,
   su paridad medida y lo que quede pendiente anotado en la etapa. Si la etapa está cerrada o no se
   puede avanzar sin una decisión mía, decilo en una línea y no hagas nada más.
2. Trabajá en una tanda larga. Mientras tanto validá sólo lo barato: compilar y los tests del
   módulo que tocás. El gate completo (`scripts/verify-rust.local.sh`) y la medición del corpus van
   una sola vez, al final de la tanda. Si la medición muestra diferencias, juntalas todas, arreglalas
   juntas y volvé a medir una vez; no midas de a un arreglo.
3. Nunca pushees sin el gate en verde. Commiteá y pusheá a `main` (con `git pull --rebase` antes),
   actualizá la etapa en `ROADMAP.md` y, si cambió cuánto le queda, el orden de las etapas.
4. Al terminar, escribí un resumen corto: qué se hizo, la paridad, y qué quedó para la próxima
   sesión. Después no sigas con otra cosa: la próxima sesión arranca limpia.

Si estás en un carril que no es `principal`, trabajás en tu propia carpeta (`../kerbo-<carril>`);
`.parity` es la de `kerbo` y los scripts la encuentran por `KERBO_PARITY`. No toques la carpeta
`kerbo` ni las etapas que otro carril tiene tomadas (`.parity/autopilot/`), y nunca crees enlaces
(junctions o symlinks) a `.parity`: borrar una carpeta con un enlace adentro borra lo enlazado.
