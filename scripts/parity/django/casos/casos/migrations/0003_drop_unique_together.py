from django.db import migrations


class Migration(migrations.Migration):
    dependencies = [("casos", "0002_big_author_ids")]

    operations = [
        migrations.AlterUniqueTogether(name="book", unique_together=set()),
        migrations.AlterUniqueTogether(name="book", unique_together={("isbn", "author")}),
    ]
