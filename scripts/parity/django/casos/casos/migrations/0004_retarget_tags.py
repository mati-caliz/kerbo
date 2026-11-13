from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("casos", "0003_drop_unique_together")]

    operations = [
        migrations.AlterField(
            model_name="book",
            name="tags",
            field=models.ManyToManyField(to="casos.label"),
        ),
        migrations.AlterField(
            model_name="book",
            name="tags",
            field=models.ManyToManyField(db_table="casos_book_labels", to="casos.label"),
        ),
    ]
