// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

class PetSearchLocalCacheMutation: LocalCacheMutation {
  static let operationType: GraphQLOperationType = .query

  var filters: GraphQLNullable<TestSchema.PetSearchFilters>

  init(filters: GraphQLNullable<TestSchema.PetSearchFilters> = .init(
    TestSchema.PetSearchFilters(
      species: ["Dog", "Cat"],
      size: .init(.small),
      measurements: .init(
        TestSchema.MeasurementsInput(
          height: 10.5,
          weight: 5.0
        )
      )
    )
  )) {
    self.filters = filters
  }

  var __variables: GraphQLOperation.Variables? { ["filters": filters] }

  struct Data: TestSchema.MutableSelectionSet {
    var __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Query }
    static var __selections: [Apollo.Selection] { [
      .field("pets", [Pet].self, arguments: ["filters": .variable("filters")]),
    ] }

    var pets: [Pet] {
      get { __data["pets"] }
      set { __data["pets"] = newValue }
    }

    init(
      pets: [Pet]
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": TestSchema.Objects.Query.typename,
          "pets": pets._fieldData,
        ],
        fulfilledFragments: [
          ObjectIdentifier(PetSearchLocalCacheMutation.Data.self)
        ]
      ))
    }

    /// Pet
    ///
    /// Parent Type: `Pet`
    struct Pet: TestSchema.MutableSelectionSet {
      var __data: DataDict
      init(_dataDict: DataDict) { __data = _dataDict }

      static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.Pet }
      static var __selections: [Apollo.Selection] { [
        .field("__typename", String.self),
        .field("id", TestSchema.ID.self),
        .field("humanName", String?.self),
      ] }

      var id: TestSchema.ID {
        get { __data["id"] }
        set { __data["id"] = newValue }
      }
      var humanName: String? {
        get { __data["humanName"] }
        set { __data["humanName"] = newValue }
      }

      init(
        __typename: String,
        id: TestSchema.ID,
        humanName: String? = nil
      ) {
        self.init(_dataDict: DataDict(
          data: [
            "__typename": __typename,
            "id": id,
            "humanName": humanName,
          ],
          fulfilledFragments: [
            ObjectIdentifier(PetSearchLocalCacheMutation.Data.Pet.self)
          ]
        ))
      }
    }
  }
}
