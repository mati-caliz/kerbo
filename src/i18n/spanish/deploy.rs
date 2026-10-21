use crate::i18n::catalog::RuleText;
use crate::rules::{ComposeRule, MigrationRule};

const UNKNOWN_MIGRATION: RuleText = RuleText::new(
    "Regla de migración",
    "seguí la indicación de la regla",
    "squawk la marca por defecto.",
    "Ver la documentación de squawk.",
);
const MIGRATION_TEXTS: [RuleText; MigrationRule::ALL.len()] = [
    RuleText::new(
        "Crea un índice sin CONCURRENTLY: bloquea las escrituras de la tabla mientras se arma",
        "creá el índice con CONCURRENTLY, en una migración que no corra dentro de una transacción",
        "Un CREATE INDEX común toma un lock SHARE: las lecturas siguen, pero cada INSERT, UPDATE y DELETE espera hasta que termine, y en una tabla grande son minutos.",
        "CREATE INDEX idx_pedidos_cliente ON pedidos (cliente_id) se vuelve CREATE INDEX CONCURRENTLY idx_pedidos_cliente ON pedidos (cliente_id); en Alembic, postgresql_concurrently=True dentro de autocommit_block().",
    ),
    RuleText::new(
        "Borra un índice sin CONCURRENTLY: bloquea lecturas y escrituras de la tabla",
        "borrá el índice con DROP INDEX CONCURRENTLY",
        "DROP INDEX toma un lock ACCESS EXCLUSIVE sobre la tabla: hasta que termina nadie la lee ni la escribe, y cada consulta que llega se encola detrás.",
        "DROP INDEX idx_viejo; se vuelve DROP INDEX CONCURRENTLY idx_viejo;",
    ),
    RuleText::new(
        "Agrega una columna NOT NULL sin default a una tabla existente",
        "agregala nullable o con un DEFAULT no volátil, y completá los datos antes de exigirla",
        "Las filas que ya existen no tienen valor: la migración falla si la tabla tiene datos, y el código desplegado que inserta sin esa columna empieza a fallar.",
        "ADD COLUMN pais text NOT NULL se vuelve ADD COLUMN pais text NOT NULL DEFAULT 'AR', o una columna nullable más un backfill.",
    ),
    RuleText::new(
        "Marca NOT NULL una columna existente: recorre la tabla entera con lecturas y escrituras bloqueadas",
        "agregá un CHECK (columna IS NOT NULL) NOT VALID, validalo aparte y recién después hacé SET NOT NULL",
        "SET NOT NULL toma ACCESS EXCLUSIVE y escanea cada fila para verificar: con una tabla grande, todo lo que la usa queda esperando.",
        "ADD CONSTRAINT t_col_no_nula CHECK (col IS NOT NULL) NOT VALID; VALIDATE CONSTRAINT t_col_no_nula; y después ALTER COLUMN col SET NOT NULL.",
    ),
    RuleText::new(
        "Cambia el tipo de una columna: reescribe la tabla con lecturas y escrituras bloqueadas",
        "agregá una columna nueva con el tipo nuevo, copiá los datos en tandas y cambiá el código antes de borrar la vieja",
        "ALTER COLUMN ... TYPE toma ACCESS EXCLUSIVE y casi siempre reescribe cada fila; además rompe a los clientes que esperan el tipo viejo.",
        "En vez de ALTER COLUMN monto TYPE numeric, sumá monto_decimal numeric, completala y pasá las lecturas a la columna nueva.",
    ),
    RuleText::new(
        "Renombra una columna que el código desplegado todavía puede usar",
        "agregá la columna nueva, escribí en las dos, pasá las lecturas y borrá la vieja en otro deploy",
        "Durante el deploy conviven la versión vieja y la nueva del código: la vieja sigue pidiendo el nombre anterior y falla apenas corre la migración.",
        "RENAME COLUMN nombre TO nombre_completo se reemplaza por ADD COLUMN nombre_completo con un backfill; el borrado de nombre va en una migración posterior.",
    ),
    RuleText::new(
        "Renombra una tabla que el código desplegado todavía puede usar",
        "creá la tabla nueva (o una vista con el nombre viejo) y sacá la vieja cuando ningún deploy la use",
        "El código que ya está corriendo sigue consultando el nombre anterior: falla desde que corre la migración hasta que termina el deploy.",
        "RENAME TO clientes se reemplaza por la tabla clientes más una vista usuarios que la lee, y la vista se borra en otra migración.",
    ),
    RuleText::new(
        "Borra una columna que el código desplegado todavía puede leer",
        "dejá de usarla en el código, desplegá, y recién después borrala",
        "Los clientes que todavía la leen o la escriben fallan en cuanto desaparece, y el borrado no se deshace sin un backup.",
        "Primero el código deja de leer descuento; en el deploy siguiente, ALTER TABLE pedidos DROP COLUMN descuento.",
    ),
    RuleText::new(
        "Borra una tabla que el código desplegado todavía puede usar",
        "dejá de usarla en el código, desplegá, y recién después borrala",
        "El código viejo sigue consultándola durante el deploy, y los datos no vuelven sin un backup.",
        "Primero se borra todo uso de auditoria_vieja; en una migración posterior, DROP TABLE auditoria_vieja.",
    ),
    RuleText::new(
        "Una operación que toma locks sin un lock_timeout antes",
        "agregá SET lock_timeout = '1s'; al principio de la migración",
        "Si la tabla está tomada, la migración espera el lock y todas las consultas que llegan detrás quedan encoladas: un ALTER trabado tira abajo la aplicación.",
        "ALTER TABLE pedidos ADD COLUMN nota text; se vuelve SET lock_timeout = '1s'; ALTER TABLE pedidos ADD COLUMN nota text;",
    ),
    RuleText::new(
        "Una operación lenta sin un statement_timeout antes",
        "agregá SET statement_timeout = '5s'; al principio de la migración",
        "Sin tope, una operación que recorre una tabla grande puede quedar minutos con el lock tomado.",
        "SET statement_timeout = '5s'; antes del primer ALTER, CREATE o DROP.",
    ),
    RuleText::new(
        "Borra una restricción que el código puede dar por garantizada",
        "dejá de depender de la restricción en el código antes de borrarla, o reemplazala por otra equivalente",
        "El código desplegado asume que la clave foránea, el UNIQUE o el CHECK siguen ahí; sin ellos entran datos que antes eran imposibles.",
        "DROP CONSTRAINT pedidos_cliente_fk va en una migración posterior a la que deja de depender de ella.",
    ),
    RuleText::new(
        "Columna timestamp sin zona horaria",
        "usá timestamptz",
        "Postgres guarda la fecha y descarta el offset: 21:00-03 y 21:00+02 quedan iguales aunque son horas distintas.",
        "creado_en timestamp se vuelve creado_en timestamptz.",
    ),
    RuleText::new(
        "Columna entera de 32 bits",
        "usá bigint (o bigserial)",
        "Un int llega a 2.147.483.647: con un id o un contador que crece, se llena y cambiar el tipo después reescribe la tabla.",
        "id serial se vuelve id bigint GENERATED BY DEFAULT AS IDENTITY.",
    ),
    RuleText::new(
        "Columna entera de 16 bits",
        "usá bigint",
        "Un smallint llega a 32.767: se llena rápido y ampliarlo después reescribe la tabla.",
        "cantidad smallint se vuelve cantidad bigint.",
    ),
    RuleText::new(
        "Columna serial en vez de una identidad",
        "usá GENERATED BY DEFAULT AS IDENTITY",
        "serial crea una secuencia aparte con permisos y dueño propios; una columna identidad es parte de la tabla y se copia y se migra con ella.",
        "id serial se vuelve id integer GENERATED BY DEFAULT AS IDENTITY.",
    ),
    RuleText::new(
        "Columna varchar con largo máximo",
        "usá text con un CHECK sobre el largo",
        "Cambiar el largo de un varchar toma ACCESS EXCLUSIVE; un CHECK se reemplaza sin bloquear la tabla.",
        "nombre varchar(255) se vuelve nombre text CHECK (char_length(nombre) <= 255).",
    ),
    RuleText::new(
        "Agrega una restricción que valida toda la tabla con escrituras bloqueadas",
        "agregala con NOT VALID y validala en otra transacción",
        "Sin NOT VALID, Postgres recorre cada fila mientras bloquea escrituras; validar aparte sólo pide un lock que deja escribir.",
        "ADD CONSTRAINT monto_positivo CHECK (monto > 0) NOT VALID; y en otra migración VALIDATE CONSTRAINT monto_positivo.",
    ),
    RuleText::new(
        "Agrega una clave foránea que bloquea escrituras en las dos tablas",
        "agregala con NOT VALID y validala en otra transacción",
        "La clave foránea toma SHARE ROW EXCLUSIVE sobre las dos tablas y las recorre: nadie escribe en ninguna hasta que termina.",
        "ADD CONSTRAINT pedidos_cliente_fk FOREIGN KEY (cliente_id) REFERENCES clientes (id) NOT VALID; y después VALIDATE CONSTRAINT.",
    ),
    RuleText::new(
        "Agrega una clave primaria que arma el índice con la tabla bloqueada",
        "creá el índice único con CONCURRENTLY y agregá la clave con USING INDEX",
        "ADD PRIMARY KEY construye el índice con ACCESS EXCLUSIVE: ni lecturas ni escrituras mientras dura.",
        "CREATE UNIQUE INDEX CONCURRENTLY pedidos_pk ON pedidos (id); ADD CONSTRAINT pedidos_pkey PRIMARY KEY USING INDEX pedidos_pk.",
    ),
    RuleText::new(
        "Agrega un UNIQUE que arma el índice con la tabla bloqueada",
        "creá el índice único con CONCURRENTLY y agregá la restricción con USING INDEX",
        "El UNIQUE construye su índice con ACCESS EXCLUSIVE: la tabla queda sin lecturas ni escrituras mientras dura.",
        "CREATE UNIQUE INDEX CONCURRENTLY usuarios_email ON usuarios (email); ADD CONSTRAINT usuarios_email_unico UNIQUE USING INDEX usuarios_email.",
    ),
    RuleText::new(
        "Agrega una columna con un default volátil o generada: reescribe la tabla",
        "agregala nullable, completala en tandas y poné el default después",
        "Desde Postgres 11 un default constante no reescribe; uno volátil (random(), gen_random_uuid()) o una columna generada sí, con ACCESS EXCLUSIVE.",
        "ADD COLUMN token uuid DEFAULT gen_random_uuid() se vuelve ADD COLUMN token uuid, un backfill y SET DEFAULT gen_random_uuid().",
    ),
    RuleText::new(
        "Agrega un valor a un enum sin BEFORE ni AFTER",
        "indicá la posición con BEFORE o AFTER",
        "Sin posición el valor va al final, y el orden del enum decide los ORDER BY y las comparaciones.",
        "ADD VALUE 'pausado' se vuelve ADD VALUE 'pausado' AFTER 'activo'.",
    ),
    RuleText::new(
        "Borra un tipo que el código o las columnas pueden usar",
        "dejá de usarlo antes de borrarlo",
        "Las columnas, funciones o clientes que lo usan fallan en cuanto desaparece.",
        "DROP TYPE estado_viejo va en una migración posterior a la que deja de usarlo.",
    ),
    RuleText::new(
        "Borra una función que el código puede llamar",
        "dejá de llamarla antes de borrarla",
        "El código desplegado que la llama falla durante el deploy.",
        "DROP FUNCTION calcular_total va después de sacar todas las llamadas.",
    ),
    RuleText::new(
        "Borra una secuencia que el código o un default pueden usar",
        "dejá de usarla antes de borrarla",
        "Un default con nextval() o un cliente que la lee falla en cuanto desaparece.",
        "DROP SEQUENCE pedidos_seq va después de sacar todos sus usos.",
    ),
    RuleText::new(
        "Borra una vista que el código puede consultar",
        "dejá de consultarla antes de borrarla",
        "El código desplegado que la lee falla durante el deploy.",
        "DROP VIEW reporte_mensual va después de sacar todas sus consultas.",
    ),
    RuleText::new(
        "Saca el default de una columna",
        "asegurate de que todo el código mande el valor antes de sacarlo",
        "Los INSERT que no mandan esa columna dejan de recibir el valor por defecto: guardan NULL o fallan.",
        "DROP DEFAULT va después de que todos los INSERT manden la columna.",
    ),
    RuleText::new(
        "Saca el NOT NULL de una columna",
        "asegurate de que el código tolere NULL antes de sacarlo",
        "El código que da por hecho que siempre hay valor empieza a recibir NULL.",
        "DROP NOT NULL va después de que todo el código maneje el NULL.",
    ),
    RuleText::new(
        "Renombra un objeto que el código desplegado puede usar",
        "creá el nuevo, pasá el código y borrá el viejo en otro deploy",
        "Durante el deploy el código viejo sigue usando el nombre anterior y falla.",
        "RENAME CONSTRAINT y RENAME TO de índices, tipos, vistas o funciones se reemplazan por crear el nuevo y borrar el viejo después.",
    ),
    RuleText::new(
        "Un nombre de más de 63 bytes",
        "acortalo",
        "Postgres lo trunca a 63 bytes sin avisar: dos nombres que difieren después del byte 63 chocan, y el código que usa el nombre completo no lo encuentra.",
        "Un índice con un nombre de 70 caracteres pasa a uno de 63 o menos.",
    ),
    RuleText::new(
        "BEGIN, COMMIT o ROLLBACK en una migración que ya corre en una transacción",
        "sacalos, o separá la migración en archivos",
        "La herramienta de migraciones ya abre la transacción: un BEGIN de más no anida, y un COMMIT corta la transacción a la mitad.",
        "Un BEGIN; ... COMMIT; dentro de una migración de Prisma o drizzle se borra.",
    ),
    RuleText::new(
        "CREATE INDEX CONCURRENTLY dentro de una transacción",
        "pasalo a una migración que no corra en una transacción",
        "Postgres no permite CONCURRENTLY dentro de una transacción: la migración falla.",
        "Mové el CREATE INDEX CONCURRENTLY a su propio archivo, sin transacción.",
    ),
    RuleText::new(
        "Una sentencia que falla si la migración se corre dos veces",
        "usá IF EXISTS o IF NOT EXISTS",
        "Fuera de una transacción, si la migración se corta a la mitad lo que ya corrió queda hecho, y al reintentarla falla en lo primero.",
        "CREATE INDEX CONCURRENTLY idx ON t (c) se vuelve CREATE INDEX CONCURRENTLY IF NOT EXISTS idx ON t (c).",
    ),
    RuleText::new(
        "Borra una base de datos entera",
        "no borres bases desde una migración",
        "Se pierden todos los datos y todo lo que la usa deja de funcionar; no hay vuelta atrás sin un backup.",
        "DROP DATABASE vieja se hace a mano, fuera de las migraciones, después de un backup.",
    ),
    RuleText::new(
        "Columna char de largo fijo",
        "usá text (o varchar sin largo)",
        "char rellena con espacios hasta el largo: las comparaciones y los largos sorprenden, y no ahorra espacio.",
        "codigo char(3) se vuelve codigo text.",
    ),
    RuleText::new(
        "TRUNCATE ... CASCADE vacía también las tablas que la referencian",
        "truncá cada tabla a propósito, sin CASCADE",
        "CASCADE sigue las claves foráneas y vacía tablas que ni se nombran en la migración.",
        "TRUNCATE pedidos CASCADE se vuelve TRUNCATE pedidos, items_pedido con las tablas explícitas.",
    ),
    RuleText::new(
        "Borra un esquema entero",
        "dejá de usarlo antes de borrarlo",
        "Todo lo que vive en el esquema desaparece y el código que lo usa falla.",
        "DROP SCHEMA viejo va después de mover o borrar a propósito lo que tiene.",
    ),
    RuleText::new(
        "Borra un dominio que las columnas pueden usar",
        "dejá de usarlo antes de borrarlo",
        "Las columnas y funciones con ese tipo fallan en cuanto desaparece.",
        "DROP DOMAIN email_t va después de cambiar las columnas que lo usan.",
    ),
    RuleText::new(
        "Borra una extensión",
        "dejá de usar sus tipos y funciones antes de borrarla",
        "Las columnas, índices y funciones que dependen de la extensión fallan o se borran con ella.",
        "DROP EXTENSION pg_trgm va después de sacar los índices que la usan.",
    ),
    RuleText::new(
        "Mueve un objeto a otro esquema",
        "creá el objeto en el esquema nuevo y pasá el código antes de sacar el viejo",
        "El código desplegado lo busca con el esquema anterior y falla.",
        "ALTER TABLE t SET SCHEMA nuevo se reemplaza por una migración en dos deploys.",
    ),
    RuleText::new(
        "Saca la expresión de una columna generada",
        "pasá el código a escribir la columna antes de sacarla",
        "La columna deja de calcularse sola: el código que sólo la leía empieza a ver valores viejos o NULL.",
        "DROP EXPRESSION va después de que el código escriba la columna.",
    ),
    RuleText::new(
        "Cambia la identidad de una columna",
        "dejá la identidad como está, o hacé el cambio con el código ya adaptado",
        "Los INSERT que mandan el valor fallan con GENERATED ALWAYS, y los que no lo mandan fallan sin identidad.",
        "ALTER COLUMN id SET GENERATED ALWAYS va después de que ningún INSERT mande el id.",
    ),
    RuleText::new(
        "Crea un dominio con restricciones",
        "poné las restricciones en las columnas, no en el dominio",
        "Cambiar después una restricción del dominio revalida cada columna de cada tabla que lo usa.",
        "CREATE DOMAIN monto AS numeric CHECK (VALUE > 0) se vuelve CREATE DOMAIN monto AS numeric y un CHECK en la columna.",
    ),
    RuleText::new(
        "Agrega una restricción a un dominio",
        "agregá la restricción en las columnas con NOT VALID",
        "ADD CONSTRAINT en un dominio recorre todas las columnas que lo usan con las tablas bloqueadas.",
        "ALTER DOMAIN monto ADD CHECK (VALUE > 0) se reemplaza por un CHECK ... NOT VALID en cada columna.",
    ),
    RuleText::new(
        "REINDEX sin CONCURRENTLY",
        "usá REINDEX ... CONCURRENTLY",
        "Un REINDEX común bloquea las escrituras de la tabla mientras reconstruye el índice.",
        "REINDEX INDEX idx se vuelve REINDEX INDEX CONCURRENTLY idx.",
    ),
    RuleText::new(
        "DETACH PARTITION sin CONCURRENTLY",
        "usá DETACH PARTITION ... CONCURRENTLY",
        "Sin CONCURRENTLY toma ACCESS EXCLUSIVE sobre la tabla particionada entera.",
        "DETACH PARTITION p2024 se vuelve DETACH PARTITION p2024 CONCURRENTLY.",
    ),
    RuleText::new(
        "Un BEGIN que nunca se cierra",
        "agregá el COMMIT al final",
        "La transacción queda abierta con sus locks hasta que se corta la conexión.",
        "BEGIN; ... se vuelve BEGIN; ... COMMIT;",
    ),
    RuleText::new(
        "La misma columna aparece dos veces",
        "dejá una sola",
        "Postgres rechaza la sentencia o se queda con un valor sin avisar cuál.",
        "INSERT INTO t (a, b, a) se vuelve INSERT INTO t (a, b).",
    ),
];

pub fn migration(rule: MigrationRule) -> RuleText {
    MIGRATION_TEXTS
        .get(rule.position())
        .copied()
        .unwrap_or(UNKNOWN_MIGRATION)
}

pub const fn compose(rule: ComposeRule) -> RuleText {
    match rule {
        ComposeRule::MemoryLimit => RuleText::new(
            "El servicio no tiene límite de memoria",
            "poné deploy.resources.limits.memory (o mem_limit) en cada servicio",
            "Sin límite, un servicio con una pérdida de memoria se come la del host y el OOM killer mata a cualquier otro, a veces la base de datos.",
            "deploy: { resources: { limits: { memory: 512M } } }",
        ),
        ComposeRule::HealthcheckStartPeriod => RuleText::new(
            "El healthcheck no tiene start_period",
            "agregá start_period con lo que tarda en arrancar el servicio",
            "Sin start_period, los chequeos que fallan durante el arranque cuentan como reintentos: un servicio lento queda unhealthy y los que dependen de él no levantan.",
            "healthcheck: { test: [\"CMD\", \"pg_isready\"], interval: 10s, timeout: 5s, retries: 5, start_period: 30s }",
        ),
        ComposeRule::HealthcheckInterval => RuleText::new(
            "El healthcheck tiene un intervalo fuera de lo razonable: entre 5 s y 5 min, con un timeout menor al intervalo",
            "usá un interval de entre 5s y 5m y un timeout menor que el interval",
            "Un intervalo muy corto carga al servicio y lo marca caído por un pico; uno muy largo tarda minutos en notar una caída; un timeout igual o mayor encima un chequeo con el siguiente.",
            "interval: 1s se vuelve interval: 10s con timeout: 5s.",
        ),
    }
}
