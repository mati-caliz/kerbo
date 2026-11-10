# Pedido de cada sesión del piloto automático

Lo lee cada sesión que abre `scripts/autopilot/sesion.ps1`. El mensaje inicial dice la etapa y el
carril; esto dice cómo trabajar.

1. Arrancá por la nota de la etapa, `$KERBO_PARITY/autopilot/notas/etapa-<N>.md` (en el carril
   principal, `.parity/autopilot/notas/`): dice dónde quedó la sesión anterior, qué archivos tocar,
   cómo medir y qué diferencias quedan con su causa. Si existe, no vuelvas a explorar lo que ya
   dice; leé de `ROADMAP.md` sólo la sección de la etapa. Si la etapa está cerrada o no se puede
   avanzar sin una decisión mía, decilo en una línea y no hagas nada más.
2. Cada avance es una tanda grande, no un arreglo: todas las diferencias que muestra la medición
   de una regla, o varias reglas juntas si son chicas. Mientras tanto validá sólo lo barato:
   compilar y los tests del módulo que tocás. El gate completo (`scripts/verify-rust.local.sh`) y
   la medición del corpus van una sola vez, al final de la tanda. Si la medición muestra
   diferencias, juntalas todas, arreglalas juntas y volvé a medir una vez; no midas de a un
   arreglo.
3. Nunca pushees sin el gate en verde. Commiteá y pusheá a `main` (con `git pull --rebase` antes),
   actualizá la etapa en `ROADMAP.md` y, si cambió cuánto le queda, el orden de las etapas.
   Reescribí la nota de la etapa para la sesión que siga: corta, sólo lo que ahorra explorar.
4. Después de pushear, **seguí con el próximo avance de la misma etapa**. Cortá sólo cuando la
   etapa quede cerrada, cuando lo que sigue pida una decisión mía, o cuando la conversación ya se
   haya compactado una vez (lo notás porque arranca con un resumen): ahí terminá el avance en curso,
   pusheá y cortá. Al cortar, escribí un resumen corto: qué se hizo, la paridad y qué quedó.
5. Sin intervención (`claude -p`) la sesión se termina en cuanto dejás de pedir herramientas. No
   cierres el turno esperando algo en segundo plano: corré lo pesado en primer plano (con el
   timeout máximo) o esperalo con `Monitor` hasta que termine.

Si estás en un carril que no es `principal`, trabajás en tu propia carpeta (`../kerbo-<carril>`);
`.parity` es la de `kerbo` y los scripts la encuentran por `KERBO_PARITY`. No toques la carpeta
`kerbo` ni las etapas que otro carril tiene tomadas (`.parity/autopilot/`), y nunca crees enlaces
(junctions o symlinks) a `.parity`: borrar una carpeta con un enlace adentro borra lo enlazado.
