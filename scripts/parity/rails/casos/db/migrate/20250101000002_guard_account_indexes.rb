class GuardAccountIndexes < ActiveRecord::Migration[7.1]
  def up
    add_index :accounts, :age, unique: true
    add_index :accounts, :email
    remove_index :accounts, :age if index_exists?(:accounts, :age, unique: false)
    remove_index :accounts, :email if index_exists?(:accounts, :email, unique: false)
    add_index :accounts, :nick, where: "nick <> ''", name: "partial_nick" unless index_exists?(:accounts, :nick, where: "nick <> ''")
  end
end
