class BulkChangeAccounts < ActiveRecord::Migration[7.1]
  def up
    change_table :accounts, bulk: true do |t|
      t.string :nick, null: false, default: ""
      t.remove :legacy
      t.change :age, :bigint
      t.change_default :name, from: nil, to: "anonymous"
      t.change_null :name, false, "anonymous"
      t.index :nick
      t.references :team, foreign_key: true
      t.timestamps null: true
      t.rename :name, :full_name
      t.string :email
      t.check_constraint "age > 0", name: "age_positive"
      t.remove_timestamps
    end
  end
end
