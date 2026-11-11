# Corre adentro del contenedor de reference.sh: aplica las migraciones de /repo/db/migrate en orden sobre
# un Postgres vacío, sólo con ActiveRecord (sin la aplicación), y guarda el SQL que manda cada una en
# /out/<migración>.sql, con BEGIN y COMMIT si corre en una transacción. Una migración que falla (usa
# modelos, helpers de la aplicación o gemas que no están) se deshace y queda anotada en errores.txt.
require "active_record"
require "fileutils"

MIGRATIONS = Dir["/repo/db/migrate/*.rb"].sort
OUT = "/out"
IGNORED_NAMES = %w[SCHEMA TRANSACTION].freeze
STATEMENT_END = ";\n"
MIGRATION_NAME = /\A(\d+_([_a-z0-9]*))/

module ApplicationHelpers
  def safety_assured
    yield
  end

  def create_trigger(*)
    Anything.new
  end
end

class Anything
  def method_missing(name, *)
    name.end_with?("?") ? false : self
  end

  def respond_to_missing?(*)
    true
  end
end

module ApplicationModels
  def const_missing(name)
    return super unless name.to_s.match?(/\A[A-Z][A-Za-z0-9]*\z/)

    model = Class.new(ActiveRecord::Base)
    model.abstract_class = name == :ApplicationRecord
    Object.const_set(name, model)
  end
end

Rails = Anything.new
ActiveRecord::Migration.include(ApplicationHelpers)
ActiveRecord::Migration.singleton_class.prepend(ApplicationModels)
ActiveRecord::Migration.verbose = false
ActiveRecord::Base.establish_connection(
  adapter: "postgresql", host: ENV.fetch("DATABASE_HOST"), username: "postgres", password: "kerbo", database: "kerbo"
)
ActiveRecord::Base.connection.execute("CREATE TABLE IF NOT EXISTS schema_migrations (version varchar PRIMARY KEY)")

captured = nil
ActiveSupport::Notifications.subscribe("sql.active_record") do |*, payload|
  next if captured.nil? || IGNORED_NAMES.include?(payload[:name])

  captured << payload[:sql]
end

def migration_class(path)
  name = ActiveSupport::Inflector.camelize(File.basename(path)[MIGRATION_NAME, 2])
  Object.send(:remove_const, name) if Object.const_defined?(name, false)
  load path
  Object.const_get(name)
end

def write(path, statements, transactional)
  body = statements.map { |statement| statement.strip.delete_suffix(";") + STATEMENT_END }.join
  body = "BEGIN#{STATEMENT_END}#{body}COMMIT#{STATEMENT_END}" if transactional
  File.write(File.join(OUT, File.basename(path)[MIGRATION_NAME, 1] + ".sql"), body)
end

errors = []
written = 0
MIGRATIONS.each do |path|
  captured = []
  version = File.basename(path)[/\A\d+/].to_i
  begin
    klass = migration_class(path)
    migration = klass.new(klass.name, version)
    transactional = !klass.disable_ddl_transaction
    if transactional
      ActiveRecord::Base.transaction { migration.migrate(:up) }
    else
      migration.migrate(:up)
    end
    statements = captured
    captured = nil
    write(path, statements, transactional)
    written += 1
  rescue StandardError, ScriptError => problem
    captured = nil
    errors << "#{File.basename(path)}: #{problem.class}: #{problem.message.lines.first&.strip}"
  end
end
File.write(File.join(OUT, "errores.txt"), errors.join("\n") + "\n")
puts "#{written} migraciones, #{errors.size} con error"
