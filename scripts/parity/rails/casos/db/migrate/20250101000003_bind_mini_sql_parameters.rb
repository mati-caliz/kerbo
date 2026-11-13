class BindMiniSqlParameters < ActiveRecord::Migration[7.1]
  def up
    DB.exec("ALTER TABLE accounts ADD COLUMN flag boolean DEFAULT ?", true)
    DB.exec("ALTER TABLE accounts ADD COLUMN note text DEFAULT :note", note: "it's")
    DB.exec(<<~SQL, limit: 10)
      ALTER TABLE accounts ADD COLUMN rank integer DEFAULT :limit CHECK (rank::integer < :limit)
    SQL
  end
end
