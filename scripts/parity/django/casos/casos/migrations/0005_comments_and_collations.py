from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("casos", "0004_retarget_tags")]

    operations = [
        migrations.AlterField(
            model_name="author",
            name="name",
            field=models.CharField(max_length=100, db_comment="Nombre público"),
        ),
        migrations.AlterField(
            model_name="author",
            name="code",
            field=models.CharField(max_length=20),
        ),
        migrations.AlterField(
            model_name="book",
            name="title",
            field=models.CharField(max_length=200, db_collation="C", db_comment="Título"),
        ),
        migrations.AddField(
            model_name="book",
            name="summary",
            field=models.TextField(default="", db_comment="Resumen"),
            preserve_default=False,
        ),
        migrations.AlterModelTableComment(name="author", table_comment="Autores"),
        migrations.AlterModelTableComment(name="book", table_comment="Libros"),
    ]
