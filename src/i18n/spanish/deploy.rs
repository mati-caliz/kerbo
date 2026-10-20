use crate::i18n::catalog::RuleText;
use crate::rules::{ComposeRule, MigrationRule};

pub const fn migration(rule: MigrationRule) -> RuleText {
    match rule {
        MigrationRule::RequireConcurrentIndexCreation => RuleText::new(
            "Crea un índice sin CONCURRENTLY: bloquea las escrituras de la tabla mientras se arma",
            "creá el índice con CONCURRENTLY, en una migración que no corra dentro de una transacción",
            "Un CREATE INDEX común toma un lock SHARE: las lecturas siguen, pero cada INSERT, UPDATE y DELETE espera hasta que termine, y en una tabla grande son minutos.",
            "CREATE INDEX idx_pedidos_cliente ON pedidos (cliente_id) se vuelve CREATE INDEX CONCURRENTLY idx_pedidos_cliente ON pedidos (cliente_id); en Alembic, postgresql_concurrently=True dentro de autocommit_block().",
        ),
        MigrationRule::RequireConcurrentIndexDeletion => RuleText::new(
            "Borra un índice sin CONCURRENTLY: bloquea lecturas y escrituras de la tabla",
            "borrá el índice con DROP INDEX CONCURRENTLY",
            "DROP INDEX toma un lock ACCESS EXCLUSIVE sobre la tabla: hasta que termina nadie la lee ni la escribe, y cada consulta que llega se encola detrás.",
            "DROP INDEX idx_viejo; se vuelve DROP INDEX CONCURRENTLY idx_viejo;",
        ),
        MigrationRule::AddingRequiredField => RuleText::new(
            "Agrega una columna NOT NULL sin default a una tabla existente",
            "agregala nullable o con un DEFAULT no volátil, y completá los datos antes de exigirla",
            "Las filas que ya existen no tienen valor: la migración falla si la tabla tiene datos, y el código desplegado que inserta sin esa columna empieza a fallar.",
            "ADD COLUMN pais text NOT NULL se vuelve ADD COLUMN pais text NOT NULL DEFAULT 'AR', o una columna nullable más un backfill.",
        ),
        MigrationRule::AddingNotNullableField => RuleText::new(
            "Marca NOT NULL una columna existente: recorre la tabla entera con lecturas y escrituras bloqueadas",
            "agregá un CHECK (columna IS NOT NULL) NOT VALID, validalo aparte y recién después hacé SET NOT NULL",
            "SET NOT NULL toma ACCESS EXCLUSIVE y escanea cada fila para verificar: con una tabla grande, todo lo que la usa queda esperando.",
            "ADD CONSTRAINT t_col_no_nula CHECK (col IS NOT NULL) NOT VALID; VALIDATE CONSTRAINT t_col_no_nula; y después ALTER COLUMN col SET NOT NULL.",
        ),
        MigrationRule::ChangingColumnType => RuleText::new(
            "Cambia el tipo de una columna: reescribe la tabla con lecturas y escrituras bloqueadas",
            "agregá una columna nueva con el tipo nuevo, copiá los datos en tandas y cambiá el código antes de borrar la vieja",
            "ALTER COLUMN ... TYPE toma ACCESS EXCLUSIVE y casi siempre reescribe cada fila; además rompe a los clientes que esperan el tipo viejo.",
            "En vez de ALTER COLUMN monto TYPE numeric, sumá monto_decimal numeric, completala y pasá las lecturas a la columna nueva.",
        ),
        MigrationRule::RenamingColumn => RuleText::new(
            "Renombra una columna que el código desplegado todavía puede usar",
            "agregá la columna nueva, escribí en las dos, pasá las lecturas y borrá la vieja en otro deploy",
            "Durante el deploy conviven la versión vieja y la nueva del código: la vieja sigue pidiendo el nombre anterior y falla apenas corre la migración.",
            "RENAME COLUMN nombre TO nombre_completo se reemplaza por ADD COLUMN nombre_completo con un backfill; el borrado de nombre va en una migración posterior.",
        ),
        MigrationRule::RenamingTable => RuleText::new(
            "Renombra una tabla que el código desplegado todavía puede usar",
            "creá la tabla nueva (o una vista con el nombre viejo) y sacá la vieja cuando ningún deploy la use",
            "El código que ya está corriendo sigue consultando el nombre anterior: falla desde que corre la migración hasta que termina el deploy.",
            "RENAME TO clientes se reemplaza por la tabla clientes más una vista usuarios que la lee, y la vista se borra en otra migración.",
        ),
        MigrationRule::BanDropColumn => RuleText::new(
            "Borra una columna que el código desplegado todavía puede leer",
            "dejá de usarla en el código, desplegá, y recién después borrala",
            "Los clientes que todavía la leen o la escriben fallan en cuanto desaparece, y el borrado no se deshace sin un backup.",
            "Primero el código deja de leer descuento; en el deploy siguiente, ALTER TABLE pedidos DROP COLUMN descuento.",
        ),
        MigrationRule::BanDropTable => RuleText::new(
            "Borra una tabla que el código desplegado todavía puede usar",
            "dejá de usarla en el código, desplegá, y recién después borrala",
            "El código viejo sigue consultándola durante el deploy, y los datos no vuelven sin un backup.",
            "Primero se borra todo uso de auditoria_vieja; en una migración posterior, DROP TABLE auditoria_vieja.",
        ),
    }
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
