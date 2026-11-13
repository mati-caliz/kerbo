from django.db import migrations, models


class Migration(migrations.Migration):
    initial = True

    dependencies = []

    operations = [
        migrations.CreateModel(
            name="Author",
            fields=[
                ("id", models.AutoField(primary_key=True, serialize=False)),
                ("name", models.CharField(max_length=100, db_comment="Nombre visible")),
                ("code", models.CharField(max_length=20, db_collation="C")),
            ],
            options={"db_table_comment": "Autores de libros"},
        ),
        migrations.CreateModel(
            name="Tag",
            fields=[
                ("id", models.AutoField(primary_key=True, serialize=False)),
                ("name", models.CharField(max_length=50)),
            ],
        ),
        migrations.CreateModel(
            name="Label",
            fields=[
                ("id", models.AutoField(primary_key=True, serialize=False)),
                ("name", models.CharField(max_length=50)),
            ],
        ),
        migrations.CreateModel(
            name="Book",
            fields=[
                ("id", models.AutoField(primary_key=True, serialize=False)),
                ("title", models.CharField(max_length=200)),
                ("isbn", models.CharField(max_length=13)),
                ("author", models.ForeignKey(on_delete=models.CASCADE, to="casos.author")),
                ("tags", models.ManyToManyField(to="casos.tag")),
            ],
            options={"unique_together": {("title", "author")}},
        ),
    ]
