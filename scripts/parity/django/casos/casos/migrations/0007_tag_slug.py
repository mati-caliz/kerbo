from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("casos", "0006_postgres_operations")]

    operations = [
        migrations.AddField(
            model_name="tag",
            name="position",
            field=models.IntegerField(db_index=True, default=0),
        ),
    ]
