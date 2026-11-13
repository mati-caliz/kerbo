from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("casos", "0001_initial")]

    operations = [
        migrations.AlterField(
            model_name="author",
            name="id",
            field=models.BigAutoField(primary_key=True, serialize=False),
        ),
        migrations.AlterField(
            model_name="tag",
            name="id",
            field=models.BigAutoField(primary_key=True, serialize=False),
        ),
    ]
