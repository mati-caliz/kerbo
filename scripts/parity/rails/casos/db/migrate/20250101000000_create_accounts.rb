class CreateAccounts < ActiveRecord::Migration[7.1]
  def change
    create_table :teams
    create_table :accounts do |t|
      t.string :name
      t.integer :age
      t.string :legacy
    end
  end
end
