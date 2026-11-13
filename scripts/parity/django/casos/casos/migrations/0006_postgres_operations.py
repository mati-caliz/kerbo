from django.contrib.postgres.operations import (
    AddConstraintNotValid,
    AddIndexConcurrently,
    BtreeGinExtension,
    CreateCollation,
    CreateExtension,
    CryptoExtension,
    RemoveCollation,
    RemoveIndexConcurrently,
    TrigramExtension,
    UnaccentExtension,
    ValidateConstraint,
)
from django.db import migrations, models


class Migration(migrations.Migration):
    atomic = False

    dependencies = [("casos", "0005_comments_and_collations")]

    operations = [
        CreateExtension("hstore"),
        BtreeGinExtension(),
        TrigramExtension(),
        CryptoExtension(),
        UnaccentExtension(),
        CreateCollation("case_insensitive", provider="icu", locale="und-u-ks-level2", deterministic=False),
        CreateCollation("german", locale="de-x-icu", provider="icu"),
        RemoveCollation("german", locale="de-x-icu", provider="icu"),
        AddIndexConcurrently("book", models.Index(fields=["isbn"], name="book_isbn_idx")),
        RemoveIndexConcurrently("book", "book_isbn_idx"),
        AddConstraintNotValid(
            "book",
            models.CheckConstraint(condition=models.Q(isbn__regex=r"^\d+$"), name="book_isbn_digits"),
        ),
        ValidateConstraint("book", "book_isbn_digits"),
    ]
