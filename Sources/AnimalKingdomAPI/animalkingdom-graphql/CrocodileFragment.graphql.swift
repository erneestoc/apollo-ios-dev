// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

struct CrocodileFragment: TestSchema.SelectionSet, Fragment {
  let __data: DataDict
  init(_dataDict: DataDict) { __data = _dataDict }

  static var __parentType: any Apollo.ParentType { TestSchema.Objects.RenamedCrocodile }
  static var __selections: [Apollo.Selection] { [
    .field("__typename", String.self),
    .field("species", String.self),
    .field("age", Int.self),
    .field("tag", String?.self, arguments: ["id": "albino"]),
  ] }

  var species: String { __data["species"] }
  var age: Int { __data["age"] }
  var tag: String? { __data["tag"] }

  init(
    species: String,
    age: Int,
    tag: String? = nil
  ) {
    self.init(_dataDict: DataDict(
      data: [
        "__typename": TestSchema.Objects.RenamedCrocodile.typename,
        "species": species,
        "age": age,
        "tag": tag,
      ],
      fulfilledFragments: [
        ObjectIdentifier(CrocodileFragment.self)
      ]
    ))
  }
}
