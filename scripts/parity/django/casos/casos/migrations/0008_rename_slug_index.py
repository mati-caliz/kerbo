from django.db import migrations


class Migration(migrations.Migration):
    dependencies = [("casos", "0007_tag_slug")]

    operations = [
        migrations.RenameIndex(model_name="tag", new_name="tag_position_idx", old_fields=("position",)),
    ]
